/* The header walk: every prototype in kui.h called once, and what comes
 * back checked — a self-test of the C surface, not an example, kept
 * beside the examples because it is the one program that links against
 * the real header (the ABI asserts the cbuild tool compiles cover the
 * structs; this covers the functions).
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

        /* A line of text runs a custom editor would draw, for the two text
         * queries: the row's key answers for every run inside it. */
        KuiSpec line = {.dir = KUI_ROW};
        /* `features` was appended to KuiTextStyle the compatible way; a
         * zeroed one is the font's defaults, this one turns ligatures off. */
        KuiTextStyle hit_mono = {.size = 16, .family = KUI_FONT_MONO,
                                 .features = KUI_STR("liga=0 calt=0")};
        kui_open_keyed(ui, KUI_STR("hitline"), &line, NULL);
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
        {KUI_STR("rich "), 0, KUI_SPAN_BOLD | KUI_SPAN_UNDERLINE, 0x3b5bd455},
        {KUI_STR("measure"), 0x73d98cff, KUI_SPAN_ITALIC | KUI_SPAN_STRIKETHROUGH, 0},
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

    /* Text queries against the frame that finished: a point becomes a byte
     * offset across the row's three runs, and a byte becomes a caret rect. */
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
    int latin = 0, physical = 0, preedits = 0, commits = 0;
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
        KuiMenuItem file[] = {
            {.label = KUI_STR("Save"), .role = KUI_MENU_CUSTOM, .enabled = 1,
             .id = save, .accel = KUI_STR("mod+s")},
            {.role = KUI_MENU_SEPARATOR},
            {.label = KUI_STR("Wrap"), .role = KUI_MENU_CUSTOM, .enabled = 1, .checked = 1},
        };
        KuiMenu menus[] = {
            {.label = KUI_STR("File"), .items = file, .count = 3, .enabled = 1},
        };
        kui_frame_begin(ui, 320, 240, 1);
        check(kui_menu_bar(ui, menus, 1), "kui_menu_bar declares and draws");
        kui_frame_finish(ui);
        kui_value_free(save);

        uint64_t rev = 0;
        check(kui_menu_bar_menu_count(ui, &rev) == 1 && rev > 0, "one menu, at a revision");
        KuiStr label = {0};
        bool on = false;
        check(kui_menu_bar_menu(ui, 0, &label, &on) == 3 && has(label, "File") && on,
              "kui_menu_bar_menu reads the title back");
        KuiStr accel = {0};
        uint32_t role = 99, flags = 0;
        check(kui_menu_bar_item(ui, 0, 2, &label, &accel, &role, &flags)
                  && has(label, "Wrap") && role == KUI_MENU_CUSTOM
                  && (flags & KUI_MENU_ITEM_CHECKED) && (flags & KUI_MENU_ITEM_ENABLED),
              "kui_menu_bar_item reads a checked row back");
        check(kui_menu_bar_item(ui, 0, 0, &label, &accel, &role, &flags) && accel.len > 0,
              "and the accelerator, in this platform's spelling");
        check(!kui_menu_bar_item(ui, 9, 9, &label, &accel, &role, &flags),
              "a row that is not there is false");

        /* The choice a native bar reports: the same event a press on the
         * drawn bar's row produces. */
        check(kui_activate_menu_bar_item(ui, 0, 0), "kui_activate_menu_bar_item");
        KuiEvent mev = KUI_EVENT_INIT;
        int menus_heard = 0;
        while (kui_poll_event(ui, &mev)) {
            const KuiValue *kind = kui_value_get(mev.payload, KUI_STR("kind"));
            KuiStr s = {0};
            if (kind && kui_value_as_str(kind, &s) && has(s, "menu")) menus_heard++;
        }
        check(menus_heard == 1, "and the app hears one menu event");
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
