/* The header walk: every prototype in kui.h called once — here, or in one
 * of the C programs beside this one (the runner and the extension
 * contract are theirs: counter.c, host.c, panel.c, conformance.c) — and
 * what comes back checked. A self-test of the C surface, not an example,
 * kept beside the examples because it is the one program that links
 * against the real header (the ABI asserts the cbuild tool compiles cover
 * the structs; this covers the functions). kui-ffi's
 * `every_entry_point_is_called` holds the claim: a prototype no C program
 * calls is a red test there (backlog AR46).
 *
 *   cargo run -p kui-devtools --bin cbuild -- --run   # what CI runs; or
 *   ./target/debug/surface          # exit 0 on a clean walk
 *
 * It walks measurement, keyed nodes and animation, hover and keyboard
 * focus, editors, accessibility, images, fonts, audio, window chrome,
 * diagnostics, values, the env setters, the theme, menus, and the readers
 * ADR 0020 added. Several checks assert on the exact colours it declared,
 * so the literals stay literal on purpose: a fixture that read the theme
 * would be asserting against whatever the host's OS happened to be set to.
 */
#include <stdlib.h>
#include <math.h>
#include "../common.h"

/* Body callback for the widgets that host custom content. */
static void body_text(void *user, KuiCtx *ui) {
    KuiTextStyle s = {.size = 12};
    kui_text(ui, KUI_STR((const char *)user), &s);
}

typedef struct Keys {
    uint64_t image;    /* in: a registered image handle */
    uint64_t sound;    /* in: a registered sound handle */
    bool claim_focus;  /* in: declare the editor focused this frame */
    uint64_t card, slider, sink, editor, input, select, drag, child; /* out: node keys */
    uint64_t checkbox, gain; /* out: the stock controls' keys */
    uint64_t hitline, region; /* out: the selectable row, the focus region */
} Keys;

static void surface_view(void *user, KuiCtx *ui) {
    Keys *k = (Keys *)user;

    KuiSpec root = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}, .gap = 8};
    kui_root(ui, &root);
    kui_window_title(ui, KUI_STR("surface"));
    kui_set_always_on_top(ui, true);
    kui_set_secure_input(ui, true);
    kui_set_option_as_alt(ui, KUI_OPTION_AS_ALT_LEFT);
    kui_set_ime_off(ui, true);

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
    /* And a drop zone (ADR 0031): the tag every phase of a file drag over
     * the card carries, and the colour the card takes while it lasts. */
    KuiValue *drop_tag = kui_value_str(KUI_STR("files"));
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
        .on_drop = drop_tag, .drop_bg = 0x2b4a50ff,
    };
    k->card = kui_open_keyed(ui, KUI_STR("card"), &card, NULL);
    kui_value_free(layout_tag); /* on_layout is cloned, not adopted */
    kui_value_free(drop_tag);   /* and so is on_drop */
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
        /* The stock select: the rows a context menu takes, the second in
         * force; the choice comes back as a menu event on this key. */
        KuiMenuItem langs[] = {
            {.label = KUI_STR("English"), .role = KUI_MENU_CUSTOM, .enabled = 1},
            {.label = KUI_STR("Deutsch"), .role = KUI_MENU_CUSTOM, .enabled = 1},
        };
        k->select = kui_select(ui, KUI_STR("language"), langs, 2, 1);
        /* The stock controls (docs/adr/0034): a mixed checkbox, a radio
         * group of two, a switch, and a slider that asks the core for its
         * changes. The tags are cloned, the click payloads consumed. */
        KuiSpec mixed = {.mixed = 1};
        k->checkbox = kui_checkbox(ui, KUI_STR("All"), &mixed, kui_value_str(KUI_STR("all")));
        kui_radio_group_open(ui, KUI_STR("Theme"), NULL);
        KuiSpec on = {.checked = 1};
        kui_radio(ui, KUI_STR("Light"), NULL, kui_value_str(KUI_STR("light")));
        kui_radio(ui, KUI_STR("Dark"), &on, kui_value_str(KUI_STR("dark")));
        kui_close(ui);
        kui_switch(ui, KUI_STR("Wi-Fi"), &on, kui_value_str(KUI_STR("wifi")));
        KuiValue *gain_tag = kui_value_str(KUI_STR("gain"));
        KuiSpec gain = {
            .value_set = KUI_VALUE_NOW | KUI_VALUE_MIN | KUI_VALUE_MAX | KUI_VALUE_STEP,
            .value_now = 40, .value_min = 0, .value_max = 100, .value_step = 5,
            .on_change = gain_tag,
        };
        k->gain = kui_slider(ui, KUI_STR("Gain"), &gain);
        kui_value_free(gain_tag);
        KuiTextStyle mono = {.size = 13, .family = KUI_FONT_MONO, .wrap = KUI_WRAP_GLYPH};
        KuiSpec editor = {
            .width = {KUI_PERCENT, 0.5f}, .height = {KUI_FIXED, 48},
            .bg = 0x11131aff, .radius = 4, .label = KUI_STR("notes"),
        };
        k->editor = kui_text_edit(ui, KUI_STR("notes"), KUI_STR("line one"), &mono,
                                  KUI_EDIT_MULTILINE, &editor);
        if (k->claim_focus) kui_set_key_focus(ui, k->editor);
        /* A field with a placeholder, shown while it is empty. */
        KuiSpec find = {
            .width = {KUI_FIXED, 120}, .height = {KUI_FIXED, 24}, .label = KUI_STR("find"),
        };
        kui_text_edit_placeholder(ui, KUI_STR("find"), KUI_STR(""), NULL, 0,
                                  KUI_STR("Search"), &find);

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

        /* A line of text runs a custom editor would draw, for the two text
         * queries: the row's key answers for every run inside it. */
        KuiSpec line = {.dir = KUI_ROW, .selectable = 1};
        /* `features` was appended to KuiTextStyle the compatible way; a
         * zeroed one is the font's defaults, this one turns ligatures off. */
        KuiTextStyle hit_mono = {.size = 16, .family = KUI_FONT_MONO,
                                 .features = KUI_STR("liga=0 calt=0")};
        k->hitline = kui_open_keyed(ui, KUI_STR("hitline"), &line, NULL);
        kui_text(ui, KUI_STR("let "), &hit_mono);
        kui_text(ui, KUI_STR("value"), &hit_mono);
        kui_text(ui, KUI_STR(" = 1;"), &hit_mono);
        kui_close(ui);

        /* A terminal's screen as one node: cells travel as a KuiCell array. */
        KuiCell screen[2 * 6] = {0};
        const char *txt = "hello!";
        for (int i = 0; i < 6; i++) {
            screen[i].ch = (uint32_t)txt[i];
            screen[i].fg = 0xd6d8e0ff;
            screen[6 + i].ch = (uint32_t)"world."[i];
            screen[6 + i].fg = 0x73d98cff;
            screen[6 + i].bg = i < 3 ? 0x1a1d27ff : 0;
        }
        screen[0].flags = KUI_CELL_BOLD;
        KuiTextStyle cell_style = {.size = 13, .family = KUI_FONT_MONO, .line_height = 18};
        KuiSpec term = {0};
        kui_cells(ui, KUI_STR("term"), 2, 6, screen, 12, &cell_style, &term, NULL, NULL, NULL,
                  1, 2, KUI_CELL_CURSOR_BLOCK, 0x6a8bffff, 0);

        /* A focus region (ADR 0022): a ring of its own, entered by name. */
        KuiSpec region = {.dir = KUI_ROW, .focus_region = 1};
        k->region = kui_open_keyed(ui, KUI_STR("dock"), &region, NULL);
        KuiSpec stop = {.width = {KUI_FIXED, 20}, .height = {KUI_FIXED, 20},
                        .bg = 0x2a2d3aff, .focusable = 1,
                        .role = KUI_ROLE_BUTTON, .label = KUI_STR("dock button")};
        kui_open_keyed(ui, KUI_STR("dock-button"), &stop, NULL);
        kui_close(ui);
        kui_close(ui);

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
    /* The audio row (ADR 0021, 6a): a fact the host pushes and a view
     * reads; a code this build has no name for reads back as closed. */
    kui_env_set_audio(ui, KUI_AUDIO_DEVICE_OPEN, 2);
    kui_env_set_audio(ui, 99, 0);
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
    /* bg was appended to KuiSpan (ABI 8): a decorated span measures like a
     * plain one, since decorations are paint. */
    KuiSpan spans[] = {
        {.text = KUI_STR("rich "), .flags = KUI_SPAN_BOLD | KUI_SPAN_UNDERLINE, .bg = 0x3b5bd455},
        {.text = KUI_STR("measure"), .color = 0x73d98cff,
         .flags = KUI_SPAN_ITALIC | KUI_SPAN_STRIKETHROUGH},
    };
    check(kui_measure_rich_text(ui, spans, 2, &body, 0, &rich), "kui_measure_rich_text");
    check(rich.width > 0 && rich.lines == 1, "spans measure as one line");

    /* Size expressions (backlog F109): built from parts or parsed, one
     * entry either way; a length reduces to KUI_FIXED, a lone percentage
     * to KUI_PERCENT, and a part that is no size makes the whole KUI_FIT. */
    KuiSizing built = kui_size_clamp(kui_size_px(400), kui_size_pct(80), kui_size_px(1000));
    KuiSizing parsed = {0};
    check(kui_size_parse(KUI_STR("clamp(400px, 80%, 1000px)"), &parsed), "kui_size_parse");
    check(built.tag == KUI_CALC && parsed.tag == KUI_CALC && built.value == parsed.value,
          "a clamp built and spelled is one calc");
    KuiSizing lengths[] = {kui_size_px(300), kui_size_px(400)};
    KuiSizing smaller = kui_size_min(lengths, 2);
    check(smaller.tag == KUI_FIXED && smaller.value == 300, "min of lengths is a length");
    KuiSizing either[] = {kui_size_pct(50), kui_size_px(300)};
    check(kui_size_max(either, 2).tag == KUI_CALC, "max over a percentage is a calc");
    KuiSizing fit = {0};
    check(kui_size_clamp(fit, kui_size_pct(50), kui_size_px(9)).tag == KUI_FIT,
          "fit is no size");
    check(!kui_size_parse(KUI_STR("clamp(1, 2)"), &parsed), "clamp takes three");

    /* Resources. Unusable input is a 0 handle, not a crash. */
    const uint8_t junk[4] = {0, 1, 2, 3};
    check(kui_font_add(ui, junk, sizeof junk) == 0, "garbage is not a font");
    check(kui_font_load_file(ui, KUI_STR("/nonexistent.ttf")) == 0, "missing font file");
    check(kui_font_load_dir(ui, KUI_STR("/nonexistent")) == 0, "missing font dir");
    check(kui_font_reload_system(ui) == 0, "a rescan with nothing installed since finds nothing");
    {
        uint64_t fb[1] = {kui_font_add_system(ui, KUI_STR("Menlo"))};
        kui_font_set_fallback(ui, fb, 1);
        kui_font_set_fallback(ui, NULL, 0); /* the platform's list alone */
    }
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
    check(kui_now(ui) == 0.05, "kui_now reads the clock kui_set_time set");
    check(isinf(kui_next_frame_at(ui)), "nothing asked for at a time yet");
    KuiEnter thrown = {0};
    thrown.set = KUI_ENTER_OFFSET | KUI_ENTER_OPACITY;
    thrown.dx = 400.0f;
    kui_exit_with(ui, kui_key_of(ui, KUI_STR("nobody")), &thrown);
    kui_exit_with(ui, kui_key_of(ui, KUI_STR("nobody")), NULL);
    kui_request_frame_at(ui, 3.0);
    check(kui_next_frame_at(ui) == 3.0, "kui_request_frame_at sets a deadline");
    /* The keyframes in the view owe their cycle; the deadline adds nothing. */
    check(!(kui_owed(ui) & KUI_OWED_REQUESTED), "and owes nothing until then");
    kui_frame_begin(ui, 800, 600, 2.0f);
    surface_view(&k, ui);
    kui_frame_finish(ui);

    check(k.checkbox && k.gain, "kui_checkbox and kui_slider return their keys");
    check(k.card && k.slider && k.sink && k.editor && k.input && k.select && k.drag && k.child,
          "every node got a key");
    check(kui_animating(ui), "the keyframes keep animating");
    /* And by kind (backlog F64): at 50 ms the card's 120 ms entrance is
     * mid-flight beside the cycle, and the two are separate bits — a wait
     * that masks the cycle off resolves once the entrance is done, where
     * kui_animating never clears. */
    check((kui_owed(ui) & (KUI_OWED_CYCLE | KUI_OWED_TRANSITION)) == (KUI_OWED_CYCLE | KUI_OWED_TRANSITION),
          "kui_owed: the cycle and the entrance, each its own bit");
    KuiStr title = {0};
    check(kui_window_title_get(ui, &title) && has(title, "surface"), "kui_window_title_get");
    /* The level: the frame asked, the host says what it did, and the two
     * are separate facts (backlog C30). */
    check(kui_always_on_top_get(ui), "kui_always_on_top_get");
    /* Secure keyboard entry is an ask the same way (backlog F85). */
    check(kui_secure_input_get(ui), "kui_secure_input_get");
    /* And the Option keys as Alt (backlog F113). */
    check(kui_option_as_alt_get(ui) == KUI_OPTION_AS_ALT_LEFT, "kui_option_as_alt_get");
    kui_env_set_always_on_top(ui, true);
    /* What is behind the window, as the host got it (backlog F126): a
     * fact like the level, read back as a view reads it; a code past the
     * end is ignored rather than trusted. */
    check(kui_ctx_backdrop(ui) == KUI_BACKDROP_OPAQUE, "a window is opaque until told");
    kui_env_set_backdrop(ui, KUI_BACKDROP_BLUR);
    kui_env_set_backdrop(ui, 99);
    check(kui_ctx_backdrop(ui) == KUI_BACKDROP_BLUR, "kui_ctx_backdrop: the blur, and not 99");

    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ui, &dd);
    check(dd.quad_count > 0 && dd.scale == 2.0f, "the surface frame drew at scale 2");
    check(dd.atlas_size > 0 && dd.atlas_pixels != NULL, "the glyph atlas is there");
    size_t images = 0;
    for (size_t i = 0; i < dd.quad_count; i++) {
        if (dd.quads[i].kind == KUI_QUAD_IMAGE) images++;
    }
    check(images > 0, "the image node drew");

    /* Text queries against the frame that finished: a point becomes a byte
     * offset across the row's three runs, and a byte becomes a caret rect. */
    check(k.sink && kui_key_named(ui, KUI_STR("key sink")) == k.sink,
          "kui_key_named finds the sink by the name a reader hears");
    check(!kui_key_named(ui, KUI_STR("sink")), "and not by its key label");
    uint64_t hitline = kui_key_of(ui, KUI_STR("hitline"));
    KuiTextHit th = KUI_TEXT_HIT_INIT;
    KuiCaretRect start = KUI_CARET_RECT_INIT, end = KUI_CARET_RECT_INIT;
    check(hitline && kui_caret_rect(ui, hitline, 0, &start) && start.h > 0 && start.w == 0,
          "kui_caret_rect at the start");
    check(kui_text_hit(ui, hitline, start.x + 1, start.y + 1, &th) && th.byte == 0 && th.line == 0,
          "kui_text_hit at the start");
    check(kui_caret_rect(ui, hitline, 999, &end) && end.x > start.x, "past the text is the end");
    check(kui_text_hit(ui, hitline, end.x + 50, start.y + 1, &th) && th.byte == 14,
          "far right of the line is its end, across the runs");
    check(kui_text_hit(ui, hitline, (start.x + end.x) / 2, start.y + 1, &th) && th.byte > 0 &&
              th.byte < 14,
          "the middle is inside");
    check(!kui_caret_rect(ui, 12345, 0, &start), "a key that drew no text answers false");
    check(!kui_ime_rect(ui, &start), "nothing with a caret is focused: no candidate window");
    check(kui_key_of(ui, KUI_STR("term")) != 0, "kui_cells keyed its node");

    /* Pointer state and scrolling. */
    kui_input_cursor(ui, 400, 300);
    kui_input_scroll(ui, 0, -3);
    kui_input_cursor_left(ui);
    kui_input_modifiers(ui, KUI_KMOD_SHIFT | KUI_KMOD_CTRL);

    /* Files dragged in from the OS (ADR 0031): over the card, which is a
     * zone, then released there. The zone under the point is what the host
     * would answer the OS with; the events land below with the rest. */
    {
        KuiLayoutRect card_rect = {.size = sizeof card_rect};
        check(kui_layout_of(ui, k.card, &card_rect), "the card has a rect");
        float cx = card_rect.x + 4, cy = card_rect.y + 4;
        KuiStr paths[2] = {KUI_STR("/drop/1.txt"), KUI_STR("/drop/2.txt")};
        kui_input_drag_files(ui, paths, 2, cx, cy);
        check(kui_drop_target(ui) == k.card, "the files are over the card");
        check(kui_is_drop_target(ui, k.card), "and the card says so");
        kui_input_drag_files(ui, paths, 2, cx + 8, cy + 8);
        kui_input_drag_files(ui, paths, 2, -50, -50);
        check(kui_drop_target(ui) == 0, "off every zone: nothing lit");
        kui_input_drag_files(ui, paths, 2, cx, cy);
        kui_input_drop_files(ui, paths, 2, cx, cy);
        check(kui_drop_target(ui) == 0, "a drop ends the hover");
        kui_input_drag_files(ui, paths, 2, cx, cy);
        kui_input_drag_cancel(ui);
        check(kui_drop_target(ui) == 0, "a cancel ends it too");
    }

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
    /* A slot with nothing loaded under its namespace is a placed, empty
     * node; kept, replayed and read back the same (ADR 0045): nothing
     * filled it, so nothing was kept, and the replay says so. */
    check(kui_slot(ui, KUI_STR("nobody/slot"), NULL), "kui_slot declares a slot nobody fills");
    check(kui_slot_kept(ui, KUI_STR("nobody/kept"), NULL), "kui_slot_kept declares one too");
    check(kui_slot_replay(ui, KUI_STR("nobody/replay"), NULL) == KUI_SLOT_NOT_KEPT,
          "kui_slot_replay with nothing kept fills fresh and says not-kept");
    check(kui_slot_fill(ui, KUI_STR("nobody/replay")) == KUI_SLOT_NOT_KEPT,
          "kui_slot_fill reads the answer back");
    check(kui_slot_fill(ui, KUI_STR("nobody/slot")) == KUI_SLOT_UNDECLARED,
          "and nothing for a slot never replayed");
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
    /* Those two calls are one half of a key each, which is what they are
     * for: a host that means to drive one channel on purpose. A host that
     * means "the user pressed this key" sends kui_input_press, which does
     * both in the order a window does them - the sink hears the press, and
     * then the core acts on it. */
    kui_focus(ui, k.sink);
    kui_input_press(ui, KUI_STR("w"), same_key, 0, no_text, false);
    kui_input_release(ui, KUI_STR("w"), same_key, 0);
    /* An IME on a sink: the composition and its commit arrive as data
     * ({kind="preedit"} then {kind="text"}), where kui_input_text would have
     * reached no sink at all. */
    kui_input_preedit(ui, KUI_STR("日本"), 0, 6);
    kui_input_commit(ui, KUI_STR("日本語"));

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
    int sliders = 0, disabled = 0, editors = 0, mixed_boxes = 0;
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
        if (n->flags & KUI_ACCESS_MIXED) mixed_boxes++;
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
    check(sliders == 2, "two sliders in the tree, one of them stock");
    check(mixed_boxes == 1, "the mixed checkbox says so");
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
    /* The other two answers a device owes (backlog F36): an imperative
     * stop that cut the sound short reports nothing - only a one-shot
     * node's removal does - and a refusal names the node that asked (the
     * root, for a kui_play) whether or not anything was tagged. */
    kui_audio_truncated(ui, playback, 0.25);
    kui_audio_refused(ui, playback);
    {
        KuiWarning refused[4];
        size_t n = kui_take_warnings(ui, refused, sizeof refused / sizeof refused[0]);
        check(n == 1 && refused[0].code.len == 16 &&
                  memcmp(refused[0].code.ptr, "playback-refused", 16) == 0,
              "a refused playback is one warning, and a truncation on an imperative stop none");
    }
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
    int latin = 0, physical = 0, preedits = 0, commits = 0;
    int drops = 0, drop_paths = 0;
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
        if (s.len == 7 && memcmp(s.ptr, "preedit", 7) == 0) preedits++;
        if (s.len == 4 && memcmp(s.ptr, "text", 4) == 0) commits++;
        if (s.len == 4 && memcmp(s.ptr, "drop", 4) == 0) {
            drops++;
            const KuiValue *paths = kui_value_get(ev.payload, KUI_STR("paths"));
            if (paths && kui_value_len(paths) == 2) drop_paths++;
        }
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
    /* Four presses through kui_input_key_down (w, its repeat, the Cyrillic
     * w, ctrl-a) and a release for each of the three distinct holds — the
     * last synthesized by letting go — plus the whole-key pair that
     * kui_input_press and kui_input_release send. The press channel is the
     * same one either way; what the press call adds is the second one. */
    check(downs == 5, "the sink took the presses, repeat included");
    check(ups == 4, "every held key came back up exactly once");
    check(latin == downs + ups,
          "every code is a Latin key, the Cyrillic press included");
    check(physical == downs + ups, "and every one carries its position");
    check(preedits == 1 && commits == 1,
          "the sink heard the composition and its commit as data");
    /* enter, move, leave, enter, drop (no leave after it), enter, leave. */
    check(drops == 7, "every phase of the file drag landed as data");
    check(drop_paths == drops, "each carrying both paths");

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

    /* The application menu bar (ADR 0018): declared in a frame, read back
     * the way a host with a bar of its own reads it, and one row chosen
     * the way that host reports a choice. */
    {
        KuiValue *save = kui_value_str(KUI_STR("file.save"));
        KuiMenuItem sort[] = {
            {.label = KUI_STR("Name"), .role = KUI_MENU_CUSTOM, .enabled = 1},
            {.label = KUI_STR("Date"), .role = KUI_MENU_CUSTOM, .enabled = 1, .checked = 1},
        };
        KuiMenuItem file[] = {
            {.label = KUI_STR("Save"), .role = KUI_MENU_CUSTOM, .enabled = 1,
             .id = save, .accel = KUI_STR("mod+s")},
            {.role = KUI_MENU_SEPARATOR},
            {.label = KUI_STR("Wrap"), .role = KUI_MENU_CUSTOM, .enabled = 1, .checked = 1},
            /* A submenu (backlog F128): rows of its own, read and chosen by
             * path. */
            {.label = KUI_STR("Sort by"), .role = KUI_MENU_CUSTOM, .enabled = 1,
             .submenu = sort, .submenu_count = 2},
        };
        KuiMenu menus[] = {
            {.label = KUI_STR("File"), .items = file, .count = 4, .enabled = 1},
        };
        kui_frame_begin(ui, 320, 240, 1);
        check(kui_menu_bar(ui, menus, 1), "kui_menu_bar declares and draws");
        kui_frame_finish(ui);
        kui_value_free(save);

        uint64_t rev = 0;
        check(kui_menu_bar_menu_count(ui, &rev) == 1 && rev > 0, "one menu, at a revision");
        KuiStr label = {0};
        bool on = false;
        check(kui_menu_bar_menu(ui, 0, &label, &on) == 4 && has(label, "File") && on,
              "kui_menu_bar_menu reads the title back");
        KuiStr accel = {0};
        uint32_t role = 99, flags = 0;
        check(kui_menu_bar_item(ui, 0, 2, &label, &accel, &role, &flags)
                  && has(label, "Wrap") && role == KUI_MENU_CUSTOM
                  && (flags & KUI_MENU_ITEM_CHECKED) && (flags & KUI_MENU_ITEM_ENABLED),
              "kui_menu_bar_item reads a checked row back");
        check(kui_menu_bar_item(ui, 0, 0, &label, &accel, &role, &flags) && accel.len > 0,
              "and the accelerator, in this platform's spelling");
#ifdef __APPLE__
        check(has(accel, "\xE2\x8C\x98S"), "mod+s reads as the bar draws it: Command-S");
#else
        check(has(accel, "Ctrl+S"), "mod+s reads as the bar draws it: Ctrl+S");
#endif
        /* The submenu, by path (backlog F128). */
        const size_t sort_row[] = {3}, date_row[] = {3, 1}, name_row[] = {3, 0};
        check(kui_menu_bar_item(ui, 0, 3, NULL, NULL, NULL, &flags)
                  && (flags & KUI_MENU_ITEM_SUBMENU),
              "a row with rows of its own says so");
        check(kui_menu_bar_submenu_count(ui, 0, NULL, 0) == 4
                  && kui_menu_bar_submenu_count(ui, 0, sort_row, 1) == 2
                  && kui_menu_bar_submenu_count(ui, 0, date_row, 2) == 0,
              "kui_menu_bar_submenu_count: the menu's rows, the submenu's, and none");
        check(kui_menu_bar_item_path(ui, 0, date_row, 2, &label, &accel, &role, &flags)
                  && has(label, "Date") && (flags & KUI_MENU_ITEM_CHECKED)
                  && !(flags & KUI_MENU_ITEM_SUBMENU),
              "kui_menu_bar_item_path reads a row inside");
        check(!kui_menu_bar_item_path(ui, 0, date_row, 2 + 1, NULL, NULL, NULL, NULL)
                  && !kui_menu_bar_item_path(ui, 0, NULL, 0, NULL, NULL, NULL, NULL),
              "through a row with no submenu, or no path, is false");
        check(!kui_activate_menu_bar_path(ui, 0, sort_row, 1),
              "the row that opens the submenu is not chosen");
        check(!kui_menu_bar_item(ui, 9, 9, &label, &accel, &role, &flags),
              "a row that is not there is false");

        /* The choice a native bar reports: the same event a press on the
         * drawn bar's row produces. */
        check(kui_activate_menu_bar_item(ui, 0, 0), "kui_activate_menu_bar_item");
        check(kui_activate_menu_bar_path(ui, 0, name_row, 2), "kui_activate_menu_bar_path");
        KuiEvent mev = KUI_EVENT_INIT;
        int menus_heard = 0, names_heard = 0;
        while (kui_poll_event(ui, &mev)) {
            const KuiValue *kind = kui_value_get(mev.payload, KUI_STR("kind"));
            const KuiValue *item = kui_value_get(mev.payload, KUI_STR("item"));
            KuiStr s = {0};
            if (kind && kui_value_as_str(kind, &s) && has(s, "menu")) menus_heard++;
            if (item && kui_value_as_str(item, &s) && has(s, "Name")) names_heard++;
        }
        check(menus_heard == 2, "and the app hears a menu event for each");
        check(names_heard == 1, "the one inside the submenu posting its own row");
    }

    /* -- The rest of the header, so that the walk is what it says it is
     * (backlog AR46): every prototype called once here or in one of the C
     * programs beside this one, held by
     * kui-ffi's `every_entry_point_is_called`. Grouped as the header is. */

    /* Values: the list half, and the readers the counter never needs. */
    {
        KuiValue *list = kui_value_list();
        kui_value_list_push(list, kui_value_bool(true));
        kui_value_list_push(list, kui_value_float(2.5));
        kui_value_list_push(list, kui_value_null());
        check(kui_value_len(list) == 3, "kui_value_len counts a list");
        bool b = false;
        double f = 0;
        check(kui_value_as_bool(kui_value_at(list, 0), &b) && b, "kui_value_at / as_bool");
        check(kui_value_as_float(kui_value_at(list, 1), &f) && f == 2.5, "kui_value_as_float");
        check(kui_value_is_null(kui_value_at(list, 2)) && kui_value_is_null(kui_value_at(list, 3)),
              "an explicit null and a missing entry read the same");
        check(!kui_value_as_bool(kui_value_at(list, 1), &b), "the readers do not coerce");
        KuiValue *map = kui_value_map();
        kui_value_map_set(map, KUI_STR("k"), kui_value_int(1));
        KuiStr key = {0};
        int64_t one = 0;
        check(kui_value_entry(map, 0, &key) && has(key, "k")
                  && kui_value_as_int(kui_value_entry(map, 0, NULL), &one) && one == 1,
              "kui_value_entry walks a map whose keys you do not know");
        check(kui_value_entry(map, 1, &key) == NULL && kui_value_len(kui_value_null()) == 0,
              "past the end is NULL, and a scalar has no entries");
        kui_value_free(list);
        kui_value_free(map);
    }

    /* The theme's two setters and the metrics: pin, read, restore. */
    {
        KuiTheme theme = KUI_THEME_INIT;
        check(kui_theme(ui, &theme), "kui_theme");
        kui_theme_set_accent(ui, 0xff8800ff);
        KuiTheme accented = KUI_THEME_INIT;
        check(kui_theme(ui, &accented) && accented.accent == 0xff8800ff, "kui_theme_set_accent");
        theme.bg = 0x102030ff;
        kui_theme_set(ui, &theme);
        KuiTheme pinned = KUI_THEME_INIT;
        check(kui_theme(ui, &pinned) && pinned.bg == 0x102030ff, "kui_theme_set pins the palette");
        kui_theme_set(ui, NULL); /* back to deriving */
        KuiMetrics metrics = KUI_METRICS_INIT;
        check(kui_metrics(ui, &metrics) && metrics.control_text > 0, "kui_metrics");
        metrics.control_text = 11;
        kui_metrics_set(ui, &metrics);
        KuiMetrics compact = KUI_METRICS_INIT;
        check(kui_metrics(ui, &compact) && compact.control_text == 11, "kui_metrics_set");
        kui_metrics_set(ui, NULL);
    }

    /* The text cache's budget and its reading; the font families. */
    kui_set_text_cache_budget(ui, 4 << 20);
    check(kui_text_cache_bytes(ui) > 0, "kui_text_cache_bytes: the frames above shaped text");
    {
        KuiStr families[4];
        size_t nf = kui_font_families(ui, families, 4);
        check(nf > 0 && families[0].len > 0, "kui_font_families lists the stock set");
        KuiSystemFont fonts[4];
        size_t ns = kui_system_fonts(ui, fonts, 4);
        check(ns == nf && fonts[0].family.len == families[0].len
                  && memcmp(fonts[0].family.ptr, families[0].ptr, families[0].len) == 0
                  && fonts[0].weight_count > 0 && fonts[0].weights[0] > 0,
              "kui_system_fonts: the same families, each with its weights");
    }

    /* The image's pixels read back, and a fragment's whole module. */
    {
        uint32_t w = 0, h = 0;
        const uint8_t *px = NULL;
        check(kui_image_pixels(ui, k.image, &w, &h, &px) && w == 2 && h == 2 && px
                  && px[0] == 255 && px[4 + 1] == 255,
              "kui_image_pixels reads the RGBA back");
        uint64_t frag = kui_fragment_add(ui, KUI_STR(
            "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n"
            "    return params[0];\n}\n"));
        check(frag != 0, "kui_fragment_add compiles a one-line fragment");
        KuiStr wgsl = {0};
        check(kui_fragment_source(ui, frag, &wgsl) && has(wgsl, "params[0]"),
              "kui_fragment_source is the module around it");
        float params[4] = {1, 0, 0, 1};
        KuiSpec fspec = {.width = {KUI_FIXED, 20}, .height = {KUI_FIXED, 20}};
        kui_frame_begin(ui, 320, 240, 1);
        kui_fragment_open_with(ui, KUI_STR("painted"), frag, 0, params, 4, &fspec);
        kui_text(ui, KUI_STR("over it"), NULL);
        kui_close(ui);
        kui_frame_finish(ui);
        KuiDrawData fdd = KUI_DRAW_DATA_INIT;
        check(kui_draw_data(ui, &fdd) && fdd.fragment_count == 1, "kui_fragment_open_with drew one");
        kui_fragment_remove(ui, frag);
        check(!kui_fragment_source(ui, frag, &wgsl), "kui_fragment_remove: the handle is dead");
    }

    /* The assistive fact, and the cursor the last hover derived. */
    kui_env_set_assistive(ui, KUI_ASSISTIVE_LISTENING);
    kui_set_time(ui, 0.3);
    kui_frame_begin(ui, 800, 600, 2.0f);
    surface_view(&k, ui);
    kui_frame_finish(ui);
    check(kui_cursor_shape(ui) != 0, "kui_cursor_shape names a cursor once a frame has hovered");

    /* The caret clock a host runs itself. */
    kui_focus(ui, k.editor);
    check(kui_has_caret(ui) && kui_caret_visible(ui), "a focused editor has a caret, shown");
    uint64_t stamp = kui_caret_stamp(ui);
    kui_set_caret_visible(ui, false);
    check(!kui_caret_visible(ui), "kui_set_caret_visible parks it");
    kui_set_caret_visible(ui, true);
    kui_input_key(ui, KUI_KEY_LEFT, 0);
    check(kui_caret_stamp(ui) != stamp, "kui_caret_stamp moves with the caret");
    kui_edit_set_text_label(ui, KUI_STR("notes"), KUI_STR("by label"));
    check(kui_edit_text(ui, k.editor, &text) && has(text, "by label"), "kui_edit_set_text_label");

    /* Layout and scrolling read back by key; a reveal resolves at the frame. */
    {
        KuiLayoutRect rect = KUI_LAYOUT_RECT_INIT;
        check(kui_layout_of(ui, k.card, &rect) && rect.w > 0 && rect.h > 0,
              "kui_layout_of answers for the on_layout node");
        check(!kui_layout_of(ui, k.sink, &rect), "and for no other");
        KuiScrollGeometry geo = KUI_SCROLL_GEOMETRY_INIT;
        check(kui_scroll_geometry(ui, k.card, &geo) && geo.h > 0, "kui_scroll_geometry on the card");
        kui_shift_scroll(ui, k.card, 0, 0); /* a zero shift moves nothing */
        kui_set_scroll(ui, k.card, 0, 9999);
        kui_reveal(ui, k.slider);
        kui_frame_begin(ui, 800, 600, 2.0f);
        surface_view(&k, ui);
        kui_frame_finish(ui);
        float sx = -1, sy = -1;
        kui_scroll_offset(ui, k.card, &sx, &sy);
        check(sx == 0 && sy >= 0, "kui_scroll_offset reads the clamped offset back");
        kui_scroll_offset(ui, 12345, NULL, &sy);
        check(sy == 0, "a node that never scrolled is 0");

        /* A scroll gesture (backlog F107): its first event picks the card
         * under the pointer, toward whichever end it has room for from where
         * it is drawn (the reveal above is still easing it), and the next
         * goes on to it. */
        KuiLayoutRect at = KUI_LAYOUT_RECT_INIT;
        check(kui_layout_of(ui, k.card, &at), "the card has a rect");
        check(kui_scroll_geometry(ui, k.card, &geo), "and a geometry");
        float before = geo.offset_y;
        float way = before >= 5 ? 1.0f : -1.0f; /* +y is toward the start */
        kui_input_cursor(ui, at.x + 4, at.y + 4);
        kui_input_scroll_gesture(ui, 0, 3 * way, true);
        kui_input_scroll_gesture(ui, 0, 2 * way, false);
        kui_frame_begin(ui, 800, 600, 2.0f);
        surface_view(&k, ui);
        kui_frame_finish(ui);
        kui_scroll_offset(ui, k.card, NULL, &sy);
        check(geo.max_offset_y < 5 || sy == before - 5 * way,
              "kui_input_scroll_gesture moves the card it began over");
    }

    /* File dialogs (backlog C51): ask, drain as the host, answer, hear it. */
    {
        KuiStr exts[] = { KUI_STR("png"), KUI_STR(".jpg") };
        KuiFileFilter filter = { KUI_STR("Images"), exts, 2 };
        KuiFileDialog dialog = { 0 };
        dialog.mode = KUI_FILE_DIALOG_OPEN;
        dialog.multiple = 1;
        dialog.title = KUI_STR("Pick images");
        dialog.filters = &filter;
        dialog.filter_count = 1;
        check(kui_request_files(ui, &dialog, kui_value_str(KUI_STR("pics"))), "kui_request_files asks");
        check(kui_awaiting_files(ui), "kui_awaiting_files while it is out");
        check(!kui_request_files(ui, NULL, NULL), "and a second ask is dropped");
        uint32_t mode = 99;
        bool multiple = false;
        KuiStr title = { 0 }, dir = { 0 }, name = { 0 };
        size_t filters = 0;
        check(kui_take_file_request(ui, &mode, &multiple, &title, &dir, &name, &filters)
                  && mode == KUI_FILE_DIALOG_OPEN && multiple && filters == 1
                  && title.len == 11 && dir.len == 0,
              "kui_take_file_request hands the host the dialog");
        KuiStr fname, fexts;
        check(kui_file_request_filter(ui, 0, &fname, &fexts) && fname.len == 6
                  && fexts.len == 7 && memcmp(fexts.ptr, "png;jpg", 7) == 0,
              "kui_file_request_filter reads a filter, dots dropped");
        check(!kui_file_request_filter(ui, 1, &fname, &fexts), "and no filter past the last");
        KuiStr picked[] = { KUI_STR("/tmp/a.png") };
        kui_input_files(ui, picked, 1);
        check(!kui_awaiting_files(ui), "kui_input_files spends the ask");
        int files = 0;
        KuiEvent fev = KUI_EVENT_INIT;
        while (kui_poll_event(ui, &fev)) {
            const KuiValue *kind = fev.payload ? kui_value_get(fev.payload, KUI_STR("kind")) : NULL;
            const KuiValue *paths = fev.payload ? kui_value_get(fev.payload, KUI_STR("paths")) : NULL;
            const KuiValue *tag = fev.payload ? kui_value_get(fev.payload, KUI_STR("tag")) : NULL;
            KuiStr ks, ts;
            if (kind && kui_value_as_str(kind, &ks) && ks.len == 5 && memcmp(ks.ptr, "files", 5) == 0
                && paths && kui_value_len(paths) == 1 && tag && kui_value_as_str(tag, &ts)
                && ts.len == 4) {
                files++;
            }
        }
        check(files == 1, "the answer is one files event with the path and the tag");
    }

    /* Documents the OS asked the app to open (backlog F124): nobody asked,
     * and the host hears them on the root; none is nothing. */
    {
        KuiStr docs[] = { KUI_STR("/tmp/a.txt"), KUI_STR("/tmp/b.md") };
        kui_input_open(ui, docs, 2);
        int opened = 0;
        KuiEvent oev = KUI_EVENT_INIT;
        while (kui_poll_event(ui, &oev)) {
            const KuiValue *kind = oev.payload ? kui_value_get(oev.payload, KUI_STR("kind")) : NULL;
            const KuiValue *paths = oev.payload ? kui_value_get(oev.payload, KUI_STR("paths")) : NULL;
            KuiStr ks, second;
            if (kind && kui_value_as_str(kind, &ks) && ks.len == 4 && memcmp(ks.ptr, "open", 4) == 0
                && paths && kui_value_len(paths) == 2
                && kui_value_as_str(kui_value_at(paths, 1), &second) && second.len == 9
                && memcmp(second.ptr, "/tmp/b.md", 9) == 0) {
                opened++;
            }
        }
        check(opened == 1, "kui_input_open is one open event with both paths");
        kui_input_open(ui, NULL, 0);
        check(!kui_poll_event(ui, &oev), "and no documents is no event");
    }

    /* Focus regions: entered by name, read back as the ring in effect. */
    check(kui_region(ui) == 0, "the main ring to begin with");
    kui_focus_region(ui, k.region);
    kui_frame_begin(ui, 800, 600, 2.0f);
    surface_view(&k, ui);
    kui_frame_finish(ui);
    check(kui_region(ui) == k.region, "kui_focus_region entered the dock's ring");
    check(kui_focused(ui) == kui_key_of(ui, KUI_STR("dock-button")), "and landed on its stop");
    kui_focus_region(ui, 0);
    kui_frame_begin(ui, 800, 600, 2.0f);
    surface_view(&k, ui);
    kui_frame_finish(ui);
    check(kui_region(ui) == 0, "and 0 is the main ring again");

    /* Selection (ADR 0017): a scope selected whole, read three ways,
     * copied, cleared; a grid's the same by lines and columns. */
    {
        check(kui_select_all_in(ui, k.hitline), "kui_select_all_in on the selectable row");
        KuiStr sel = {0}, html = {0};
        check(kui_selection_text(ui, &sel) && has(sel, "let value = 1;"), "kui_selection_text");
        check(kui_selection_html(ui, &html) && html.len > 0, "kui_selection_html");
        int64_t ai = 5, fi = 5;
        size_t ab = 1, fb = 0;
        check(kui_selection_ends(ui, &ai, &ab, &fi, &fb) && ai == -1 && fi == -1 && ab == 0 && fb > 0,
              "kui_selection_ends: outside every virtual row, from the start to the end");
        KuiStr copied = {0};
        check(kui_request_copy(ui, &copied) == KUI_COPY_READY && has(copied, "value"),
              "kui_request_copy is ready with the text");
        check(!kui_answer_selection_range(ui, KUI_STR("late")), "nothing asked, so no answer taken");
        check(kui_clear_selection(ui), "kui_clear_selection");
        check(!kui_selection_text(ui, &sel), "and there is none");
        uint64_t term = kui_key_of(ui, KUI_STR("term"));
        check(kui_select_all_in(ui, term), "kui_select_all_in on the grid");
        uint64_t node = 0, al = 9, fl = 0;
        size_t ac = 9, fc = 0;
        bool block = true;
        check(kui_cell_selection(ui, &node, &al, &ac, &fl, &fc, &block) && node == term
                  && al == 0 && ac == 0 && fl == 1 && fc == 6 && !block,
              "kui_cell_selection: the whole screen, linewise");
        check(kui_clear_selection(ui) && !kui_cell_selection(ui, NULL, NULL, NULL, NULL, NULL, NULL),
              "cleared, a grid's selection reads false");
    }

    /* The clipboard the app owns, drained as menu actions. */
    {
        kui_set_lookup_available(ui, true);
        kui_set_clipboard(ui, KUI_STR("plain"), KUI_STR("<b>plain</b>"));
        check(!kui_awaiting_paste(ui), "no paste asked yet");
        kui_request_paste(ui);
        kui_request_paste(ui); /* one ask at a time: dropped (AR34) */
        check(kui_awaiting_paste(ui), "kui_awaiting_paste while one is out");
        KuiMenuAction act = KUI_MENU_ACTION_INIT;
        check(kui_take_menu_action(ui, &act) && act.kind == KUI_MENU_ACTION_SET_CLIPBOARD
                  && has(act.text, "plain") && has(act.html, "<b>"),
              "kui_set_clipboard queues both flavours");
        check(kui_take_menu_action(ui, &act) && act.kind == KUI_MENU_ACTION_PASTE,
              "kui_request_paste queues the ask");
        check(!kui_take_menu_action(ui, &act), "drained, and the second ask was dropped");
        check(kui_awaiting_paste(ui), "still out until answered");
        kui_input_commit(ui, KUI_STR("")); /* the clipboard held nothing */
        check(!kui_awaiting_paste(ui), "an empty commit is an answer");
        /* A secret goes out as its own kind, for the host to write marked
         * concealed and transient; a marked paste is an answer too
         * (backlog F84). */
        kui_set_clipboard_secret(ui, KUI_STR("hunter2"));
        check(kui_take_menu_action(ui, &act) && act.kind == KUI_MENU_ACTION_SET_CLIPBOARD_SECRET
                  && has(act.text, "hunter2") && act.html.len == 0,
              "kui_set_clipboard_secret queues the secret alone");
        kui_request_paste(ui);
        check(kui_take_menu_action(ui, &act) && act.kind == KUI_MENU_ACTION_PASTE, "asked again");
        kui_input_paste(ui, KUI_STR("s3cret"), KUI_PASTE_CONCEALED | KUI_PASTE_TRANSIENT);
        check(!kui_awaiting_paste(ui), "a marked paste is an answer");
    }

    /* A context menu the host shows itself: opened over a node, read row
     * by row, chosen, and one closed unchosen. */
    {
        kui_set_native_menus(ui, true);
        kui_set_native_menu_bar(ui, true);
        KuiMenuItem rows[] = {
            {.label = KUI_STR("Rename"), .role = KUI_MENU_CUSTOM, .enabled = 1},
            {.role = KUI_MENU_SEPARATOR},
            {.role = KUI_MENU_COPY, .enabled = 1},
        };
        check(kui_open_menu(ui, k.card, 10, 20, rows, 3), "kui_open_menu");
        uint64_t target = 0;
        float mx = 0, my = 0;
        check(kui_menu_item_count(ui, &target, &mx, &my) == 3 && target == k.card && mx == 10 && my == 20,
              "kui_menu_item_count: the rows, the node and the point");
        KuiStr label = {0}, accel = {0};
        uint32_t role = 99, flags = 0;
        check(kui_menu_item(ui, 0, &label, &accel, &role, &flags) && has(label, "Rename")
                  && role == KUI_MENU_CUSTOM && (flags & KUI_MENU_ITEM_ENABLED),
              "kui_menu_item reads a row back");
        check(kui_menu_item(ui, 2, &label, &accel, &role, NULL) && role == KUI_MENU_COPY && label.len > 0,
              "a standard role carries its own label");
        check(!kui_menu_item(ui, 3, NULL, NULL, NULL, NULL), "past the end is false");
        check(kui_activate_menu_item(ui, 0), "kui_activate_menu_item chooses the row");
        check(kui_menu_item_count(ui, NULL, NULL, NULL) == 0, "which closed the menu");
        KuiEvent mev = KUI_EVENT_INIT;
        int chosen = 0;
        while (kui_poll_event(ui, &mev)) {
            const KuiValue *kind = mev.payload ? kui_value_get(mev.payload, KUI_STR("kind")) : NULL;
            KuiStr ks = {0};
            if (kind && kui_value_as_str(kind, &ks) && has(ks, "menu") && mev.key == k.card) chosen++;
        }
        check(chosen == 1, "and the app heard it on the node");
        check(kui_open_menu(ui, k.card, 0, 0, rows, 3) && kui_close_menu(ui), "kui_close_menu");
        check(!kui_close_menu(ui), "false when nothing was open");
        check(!kui_open_menu(ui, 0, 0, 0, rows, 3), "a key of 0 opens nothing");
        kui_set_native_menus(ui, false);
        kui_set_native_menu_bar(ui, false);
    }

    /* The menu the core draws (backlog F127, F128): an accelerator read
     * back as it is drawn, in the platform's spelling; the menu as wide as
     * its widest row, which stays one line; and a submenu read and chosen
     * by its path. */
    {
        KuiMenuItem sort[] = {
            {.label = KUI_STR("Name"), .role = KUI_MENU_CUSTOM, .enabled = 1},
            {.label = KUI_STR("Date"), .role = KUI_MENU_CUSTOM, .enabled = 1, .checked = 1},
        };
        KuiMenuItem rows[] = {
            {.label = KUI_STR("Open"), .role = KUI_MENU_CUSTOM, .enabled = 1,
             .accel = KUI_STR("mod+o")},
            {.label = KUI_STR("Sort by"), .role = KUI_MENU_CUSTOM, .enabled = 1,
             .submenu = sort, .submenu_count = 2},
        };
        KuiMenuItem wide[] = {
            {.label = KUI_STR("Open"), .role = KUI_MENU_CUSTOM, .enabled = 1,
             .accel = KUI_STR("mod+o")},
            {.label = KUI_STR("Move the note to the trash, for good"),
             .role = KUI_MENU_CUSTOM, .enabled = 1,
             .accel = KUI_STR("ctrl+alt+shift+backspace")},
        };
        KuiAccessNode an[256];
        /* The row named `name` in the drawn menu: its width and height. */
#define MENU_ROW(NAME, OUT_W, OUT_H)                                           \
    do {                                                                       \
        size_t got_ = kui_access_tree(ui, an, sizeof an / sizeof an[0]);       \
        for (size_t i_ = 0; i_ < got_; i_++)                                   \
            if (an[i_].role == KUI_ROLE_MENU_ITEM && has(an[i_].name, NAME)) { \
                OUT_W = an[i_].w;                                              \
                OUT_H = an[i_].h;                                              \
            }                                                                  \
    } while (0)
        check(kui_open_menu(ui, k.card, 10, 10, rows, 2), "a menu with a submenu opens");
        kui_frame_begin(ui, 800, 600, 1.0f);
        surface_view(&k, ui);
        kui_frame_finish(ui);
        float narrow_w = 0, open_h = 0, sort_w = 0, sort_h = 0;
        MENU_ROW("Open", narrow_w, open_h);
        MENU_ROW("Sort by", sort_w, sort_h);
        check(narrow_w > 0 && narrow_w == sort_w, "every row as wide as the menu");
        KuiStr label = {0}, accel = {0};
        uint32_t role = 99, flags = 0;
        check(kui_menu_item(ui, 0, &label, &accel, &role, &flags) && accel.len > 0,
              "the row's accelerator reads back");
#ifdef __APPLE__
        check(has(accel, "\xE2\x8C\x98O"), "mod+o in the platform's spelling: Command-O");
#else
        check(has(accel, "Ctrl+O"), "mod+o in the platform's spelling: Ctrl+O");
#endif
        check(kui_menu_item(ui, 1, NULL, NULL, NULL, &flags) && (flags & KUI_MENU_ITEM_SUBMENU),
              "the submenu's row says it has rows");
        const size_t sort_row[] = {1}, date_row[] = {1, 1}, name_row[] = {1, 0};
        check(kui_menu_submenu_count(ui, NULL, 0) == 2 && kui_menu_submenu_count(ui, sort_row, 1) == 2,
              "kui_menu_submenu_count: the menu's rows and the submenu's");
        check(kui_menu_item_path(ui, date_row, 2, &label, NULL, &role, &flags) && has(label, "Date")
                  && (flags & KUI_MENU_ITEM_CHECKED),
              "kui_menu_item_path reads a row inside");
        check(!kui_activate_menu_path(ui, sort_row, 1) && kui_menu_item_count(ui, NULL, NULL, NULL) == 2,
              "the row that opens the submenu is not chosen, and the menu stays open");
        check(kui_activate_menu_path(ui, name_row, 2), "kui_activate_menu_path chooses the row inside");
        check(kui_menu_item_count(ui, NULL, NULL, NULL) == 0, "which closed the menu");
        KuiEvent mev = KUI_EVENT_INIT;
        int named = 0;
        while (kui_poll_event(ui, &mev)) {
            const KuiValue *item = mev.payload ? kui_value_get(mev.payload, KUI_STR("item")) : NULL;
            KuiStr s = {0};
            if (item && kui_value_as_str(item, &s) && has(s, "Name") && mev.key == k.card) named++;
        }
        check(named == 1, "and the app heard the row inside, on the node");

        check(kui_open_menu(ui, k.card, 10, 10, wide, 2), "a menu with a long row opens");
        kui_frame_begin(ui, 800, 600, 1.0f);
        surface_view(&k, ui);
        kui_frame_finish(ui);
        float wide_w = 0, long_w = 0, long_h = 0, wide_open_h = 0;
        MENU_ROW("Open", wide_w, wide_open_h);
        MENU_ROW("Move the note to the trash, for good", long_w, long_h);
        check(wide_w > narrow_w && wide_w == long_w, "the menu widens to its widest row");
        check(long_h == wide_open_h && wide_open_h == open_h, "and the long row is one line");
#undef MENU_ROW
        check(kui_close_menu(ui), "closed");
    }

    /* The devtools doors (ADR 0024): the panel, its dock, its theme,
     * legend and inspect chord, and the node snapshot a tree view reads. */
    {
        check(!kui_devtools(ui), "the panel is off until asked");
        kui_set_devtools(ui, true);
        check(kui_devtools(ui), "kui_set_devtools");
        check(kui_set_devtools_dock(ui, KUI_STR("left")), "kui_set_devtools_dock");
        check(!kui_set_devtools_dock(ui, KUI_STR("sideways")), "a dock word this build lacks is false");
        KuiStr dock = {0};
        check(kui_devtools_dock(ui, &dock) && has(dock, "left"), "kui_devtools_dock reads it back");
        check(kui_set_devtools_theme(ui, KUI_STR("dark"), 0x3b5bd4ff), "kui_set_devtools_theme");
        check(!kui_set_devtools_theme(ui, KUI_STR("blue"), 0), "a base that is not one is false");
        KuiStr keys[] = {KUI_STR("Space")};
        KuiStr what[] = {KUI_STR("play")};
        kui_set_devtools_legend(ui, keys, what, 1);
        KuiStr chord = {0};
        check(kui_devtools_key(ui, &chord) && has(chord, "ctrl+shift+i"), "kui_devtools_key: the default");
        check(kui_set_devtools_key(ui, KUI_STR("f12")), "kui_set_devtools_key");
        check(!kui_set_devtools_key(ui, KUI_STR("f99")), "a chord kui cannot name is false");
        check(kui_devtools_key(ui, &chord) && has(chord, "f12"), "the respelled chord reads back");
        check(kui_devtools_selected(ui) == 0 && kui_devtools_hovered(ui) == 0 && kui_devtools_picked(ui) == 0,
              "nothing selected, hovered or picked yet");
        kui_set_devtools_selected(ui, 0);
        kui_set_devtools_pick(ui, true);
        check(kui_devtools_picking(ui), "kui_set_devtools_pick raises the picker");
        kui_set_devtools_pick(ui, false);
        check(!kui_devtools_picking(ui), "and puts it away");
        KuiStr tab = {0};
        check(kui_devtools_current_tab(ui, &tab) && has(tab, "tree"), "kui_devtools_current_tab: the pick showed the tree");
        check(kui_set_devtools_tab(ui, KUI_STR("events")), "kui_set_devtools_tab: one of the panel's own");
        check(kui_devtools_current_tab(ui, &tab) && has(tab, "events"), "and it reads back");
        check(!kui_set_devtools_tab(ui, KUI_STR("mine")), "a declared name is listed from the panel's first frame on, not before");
        kui_set_inspect(ui, true);
        kui_frame_begin(ui, 800, 600, 2.0f);
        surface_view(&k, ui);
        /* A declared tab of each form (ADR 0032): the extension form names a
         * slot nobody loaded (harmless: not on show, and a tab's slot raises
         * no unknown-slot); the host form's open answers false since the
         * name kui_set_devtools_tab selected above is listed only from the
         * panel's next frame, so its body is skipped. A second declaration
         * of a name is refused. */
        check(kui_devtools_tab(ui, KUI_STR("plug"), KUI_STR("Plugin"), KUI_STR("ts/panel")), "kui_devtools_tab");
        check(!kui_devtools_tab(ui, KUI_STR("plug"), KUI_STR("Again"), KUI_STR("ts/again")), "a name twice is refused");
        if (kui_devtools_tab_open(ui, KUI_STR("mine"), KUI_STR("Mine"))) {
            check(false, "kui_devtools_tab_open: not on show, so nothing opens");
            kui_close(ui);
        } else {
            check(true, "kui_devtools_tab_open answers false off show");
        }
        kui_frame_finish(ui);
        /* Where the frame put the host: right of the left dock, the rest of
         * the 800x600 window (backlog F92). */
        KuiLayoutRect host = KUI_LAYOUT_RECT_INIT;
        check(kui_host_rect(ui, &host) && host.x > 0 && host.x + host.w == 800 && host.y == 0 && host.h == 600,
              "kui_host_rect: the host area beside the dock");
        check(!kui_host_rect(ui, NULL), "and false for a NULL out");
        {
            KuiWarning dup[8];
            size_t n = kui_take_warnings(ui, dup, sizeof dup / sizeof dup[0]);
            int seen = 0;
            for (size_t i = 0; i < n; i++) {
                if (dup[i].code.len == 13 && memcmp(dup[i].code.ptr, "duplicate-tab", 13) == 0) seen++;
            }
            check(seen == 1 && n == 1, "the second declaration is the duplicate-tab diagnostic, and nothing else");
        }
        const KuiValue *nodes_list = kui_nodes(ui);
        check(nodes_list && kui_value_len(nodes_list) > 1, "kui_nodes: the frame's nodes, as data");
        const KuiValue *first_node = kui_value_at(nodes_list, 0);
        check(first_node && kui_value_get(first_node, KUI_STR("key")) && kui_value_get(first_node, KUI_STR("kind")),
              "each with a key and a kind");
        kui_set_inspect(ui, false);
        kui_set_devtools(ui, false);
        check(!kui_devtools(ui), "and off again");
        KuiWindowCommand cmd = KUI_WINDOW_COMMAND_INIT;
        while (kui_take_window_command(ui, &cmd)) {}
        KuiEvent dev = KUI_EVENT_INIT;
        while (kui_poll_event(ui, &dev)) {}
    }

    /* Why a frame runs (backlog F111), on a context of its own: the
     * input handed in and the host's note are the next frame's reasons,
     * KUI_FRAME_CAUSE_OWED the one after a frame that owed another, and,
     * traced, whether a frame drew what the one before drew. */
    {
        KuiCtx *why = kui_ctx_new();
        kui_set_frame_trace(why, true);
        kui_frame_begin(why, 320, 240, 1);
        kui_frame_finish(why);
        check(kui_frame_unchanged(why) == -1, "kui_frame_unchanged: nothing to compare the first frame with");
        kui_input_cursor(why, 5, 5);
        kui_note_frame_cause(why, KUI_FRAME_CAUSE_WAKE);
        kui_frame_begin(why, 320, 240, 1);
        check(kui_frame_cause(why) == (KUI_FRAME_CAUSE_POINTER_MOVE | KUI_FRAME_CAUSE_WAKE),
              "kui_frame_cause: the pointer's move and the host's wake");
        kui_frame_finish(why);
        check(kui_frame_unchanged(why) == 1, "and it drew what the frame before drew");
        kui_ctx_free(why);
    }

    /* A standalone context is nobody's slot. */
    {
        KuiStr name = {0};
        check(!kui_slot_name(ui, &name) && !kui_slot_namespace(ui, &name),
              "kui_slot_name / kui_slot_namespace are false outside an extension");
    }

    /* And none of that raised a diagnostic either. */
    {
        size_t late = kui_take_warnings(ui, warnings, sizeof warnings / sizeof warnings[0]);
        for (size_t i = 0; i < late; i++) {
            fprintf(stderr, "  warning: %.*s: %.*s\n",
                    (int)warnings[i].code.len, (const char *)warnings[i].code.ptr,
                    (int)warnings[i].message.len, (const char *)warnings[i].message.ptr);
        }
        check(late == 0, "the rest of the walk raises no diagnostics");
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
 *     ./target/debug/counter --conformance target/conformance.txt
 *
 * The report format is documented on `conformance::report`; it carries no
 * formatted floats (quad geometry travels as one FNV-1a digest) precisely
 * so C and Rust and JavaScript can print the same thing.
 */

int main(void) {
    if (!abi_ok()) return 1;
    return surface();
}
