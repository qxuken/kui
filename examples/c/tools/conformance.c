/* The C adapter over the scene corpus (kui-core::conformance): every
 * scene rebuilt through the public C API alone, its report diffed against
 * the reference kui-core wrote — so the four bindings are held to one
 * lowering. A tool, not an example; it is here because it is the one
 * corpus adapter written in C.
 *
 *   cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt
 *   ./target/debug/conformance [target/conformance.txt]
 *
 * The reference is generated, never checked in: the quad digests cover
 * real glyph geometry, so they hold only for the machine and fonts that
 * produced them. Exit 1 when it cannot be read, so a round that lost the
 * step producing it goes red rather than skipping.
 */
#include <stdarg.h>
#include <stdlib.h>
#include "../common.h"

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

/* FNV-1a over each quad's words 0..18 - KuiQuad without its uv, which
 * follows glyph insertion order, and without the clip index - plus the uv
 * of a KUI_QUAD_SEGMENT, where it is the endpoints, or of a
 * KUI_QUAD_TEXTURE, where it is the side-list index, and then the eight
 * words of the clip that index names. The clip is digested resolved, not
 * as the index, so the number says what a backend clips by and not how the
 * frame interned it. Mirrors conformance::quad_digest. */
_Static_assert(sizeof(KuiQuad) == 24 * sizeof(uint32_t), "KuiQuad is not 24 words");
_Static_assert(sizeof(KuiClip) == 8 * sizeof(uint32_t), "KuiClip is not 8 words");

static void digest_words(uint64_t *h, const uint32_t *w, int n) {
    for (int j = 0; j < n; j++) {
        uint32_t v = w[j];
        for (int b = 0; b < 4; b++) {
            *h ^= (uint8_t)(v & 0xff);
            *h *= 0x100000001b3ull;
            v >>= 8;
        }
    }
}

static uint64_t quad_digest(const KuiQuad *quads, size_t count,
                            const KuiClip *clips, size_t clip_count) {
    uint64_t h = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < count; i++) {
        uint32_t w[24];
        memcpy(w, &quads[i], sizeof w);
        digest_words(&h, w, 19); /* x..kind; word 19 is the clip index */
        if (quads[i].kind == KUI_QUAD_SEGMENT || quads[i].kind == KUI_QUAD_TEXTURE) {
            digest_words(&h, w + 20, 4);
        }
        uint32_t c[8] = {0};
        if (quads[i].clip < clip_count) memcpy(c, &clips[quads[i].clip], sizeof c);
        digest_words(&h, c, 8);
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
        "radioGroup", "menu", "menuItem", "terminal",
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
    /* conformance::Fixtures::stream: a copy of the image, updated in place
     * to 8x2 grey before the first frame, so it is texture-backed. */
    uint64_t stream;
    uint64_t sound;
    uint64_t fragment;
    /* conformance::Fixtures::sampler: the one that reads its image. */
    uint64_t sampler;
    /* conformance::Fixtures::dead: an image registered and removed, the
     * handle the dead-handle rule is pinned on. Removed rather than made
     * up: 0 is "no image" at this door, and any other number is the first
     * session's handle in some process (C31). */
    uint64_t dead;
} Fixtures;

/* conformance::FRAGMENT_WGSL, character for character. */
static const char *CONF_FRAGMENT_WGSL =
    "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n"
    "    let t = clamp(in.local.y / max(in.size.y, 1.0), 0.0, 1.0);\n"
    "    let base = mix(params[0], params[1], t);\n"
    "    let d = kui_sd_rounded_box(in.local - in.size * 0.5, in.size * 0.5, vec4<f32>(params[2].x));\n"
    "    let ring = 1.0 - smoothstep(-KUI_AA, KUI_AA, abs(d) - params[2].y);\n"
    "    return vec4<f32>(mix(base.rgb, params[3].rgb, ring), base.a);\n"
    "}";

/* conformance::FRAGMENT_IMAGE_WGSL, character for character. */
static const char *CONF_FRAGMENT_IMAGE_WGSL =
    "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n"
    "    let uv = in.local / max(in.size, vec2<f32>(1.0));\n"
    "    let c = kui_sample(uv) * params[0];\n"
    "    return vec4<f32>(c.rgb, c.a * step(1.0, in.image.z));\n"
    "}";

/* conformance::FRAGMENT_IMAGE_PARAMS. */
static const float CONF_FRAGMENT_IMAGE_PARAMS[4] = {1.0f, 0.5f, 0.25f, 1.0f};

/* conformance::FRAGMENT_PARAMS and FRAGMENT_PARAMS_LONG. */
static const float CONF_FRAGMENT_PARAMS[16] = {
    0.85f, 0.30f, 0.25f, 1.0f,
    0.20f, 0.45f, 0.90f, 1.0f,
    10.0f, 2.0f, 0.0f, 0.0f,
    1.0f, 1.0f, 1.0f, 1.0f,
};
static const float CONF_FRAGMENT_PARAMS_LONG[18] = {
    0.1f, 0.2f, 0.3f, 1.0f, 0.4f, 0.5f, 0.6f, 1.0f, 4.0f,
    1.0f, 0.0f, 0.0f, 0.9f, 0.9f, 0.2f, 1.0f, 7.0f, 8.0f,
};

static Fixtures conf_fixtures(KuiCtx *ctx) {
    uint8_t rgba[4 * 4 * 4];
    memset(rgba, 0xff, sizeof rgba);
    Fixtures f;
    f.image = kui_image_add(ctx, 4, 4, rgba);
    f.stream = kui_image_add(ctx, 4, 4, rgba);
    uint8_t grey[8 * 2 * 4];
    for (int i = 0; i < 8 * 2 * 4; i++) grey[i] = (i % 4 == 3) ? 0xff : 0x80;
    kui_image_update(ctx, f.stream, 8, 2, grey);
    f.sound = kui_sound_add(ctx, (const uint8_t *)"RIFF....WAVE", 12);
    f.fragment = kui_fragment_add(ctx, KUI_STR(CONF_FRAGMENT_WGSL));
    f.sampler = kui_fragment_add(ctx, KUI_STR(CONF_FRAGMENT_IMAGE_WGSL));
    f.dead = kui_image_add(ctx, 4, 4, rgba);
    kui_image_remove(ctx, f.dead);
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
        {.text = KUI_STR("a ")},
        {.text = KUI_STR("b"), .color = 0x73d98cff, .flags = KUI_SPAN_BOLD},
        {.text = KUI_STR(" c"), .flags = KUI_SPAN_ITALIC},
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

/* conformance::build_align: the three spreads (backlog C13), a baseline row
 * of a 12 and a 20 px text and a box, and a ratio sizing each axis (C14). */
static void conf_align_square(KuiCtx *ui) {
    KuiSpec sq = {.width = {KUI_FIXED, 10}, .height = {KUI_FIXED, 10}, .bg = 0x30344aff};
    kui_open(ui, &sq, NULL);
    kui_close(ui);
}

static void conf_align(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    static const uint32_t spreads[3] = {KUI_SPACE_BETWEEN, KUI_SPACE_AROUND, KUI_SPACE_EVENLY};
    KuiSpec outer = {
        .pad_l = 4, .pad_r = 4, .pad_t = 4, .pad_b = 4, .gap = 6,
        .width = {KUI_FIXED, 128}, .bg = 0x101018ff,
    };
    kui_open(ui, &outer, NULL);
    for (int i = 0; i < 3; i++) {
        KuiSpec row = {.dir = KUI_ROW, .width = {KUI_FIXED, 120}, .main_align = spreads[i]};
        kui_open(ui, &row, NULL);
        for (int k = 0; k < 3; k++) conf_align_square(ui);
        kui_close(ui);
    }
    KuiSpec base = {.dir = KUI_ROW, .gap = 4, .cross_align = KUI_BASELINE};
    kui_open(ui, &base, NULL);
    KuiTextStyle s12 = {.size = 12}, s20 = {.size = 20};
    kui_text(ui, KUI_STR("ab"), &s12);
    kui_text(ui, KUI_STR("cd"), &s20);
    conf_align_square(ui);
    kui_close(ui);
    KuiSpec wide = {.width = {KUI_GROW, 1}, .aspect_ratio = 4, .bg = 0x3b5bd4ff};
    kui_open(ui, &wide, NULL);
    kui_close(ui);
    KuiSpec tall = {.height = {KUI_FIXED, 12}, .aspect_ratio = 2, .bg = 0x73d98cff};
    kui_open(ui, &tall, NULL);
    kui_close(ui);
    kui_close(ui);
}

/* conformance::build_stock_controls (docs/adr/0034): every toggle keyed
 * by its text, as kui_checkbox keys it. */
static KuiValue *conf_kind(const char *k) {
    KuiValue *m = kui_value_map();
    kui_value_map_set(m, KUI_STR("kind"), kui_value_str((KuiStr){(const uint8_t *)k, strlen(k)}));
    return m;
}

static void conf_stock_controls(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 8, .pad_r = 8, .pad_t = 8, .pad_b = 8, .gap = 8};
    kui_open(ui, &outer, NULL);
    KuiValue *vol = conf_kind("vol");
    KuiSpec slider = {
        .width = {KUI_FIXED, 216},
        .value_set = KUI_VALUE_NOW | KUI_VALUE_MIN | KUI_VALUE_MAX | KUI_VALUE_STEP,
        .value_now = 30, .value_min = 0, .value_max = 100, .value_step = 10,
        .on_change = vol,
    };
    kui_slider(ui, KUI_STR("Volume"), &slider);
    kui_value_free(vol);
    kui_checkbox(ui, KUI_STR("Mute"), NULL, conf_kind("mute"));
    KuiSpec on = {.checked = 1};
    kui_checkbox(ui, KUI_STR("Sync"), &on, conf_kind("sync"));
    KuiSpec mixed = {.mixed = 1};
    kui_checkbox(ui, KUI_STR("All"), &mixed, conf_kind("all"));
    kui_radio_group_open(ui, KUI_STR("Theme"), NULL);
    kui_radio(ui, KUI_STR("Light"), NULL, conf_kind("light"));
    kui_radio(ui, KUI_STR("Dark"), &on, conf_kind("dark"));
    kui_close(ui);
    kui_switch(ui, KUI_STR("Wi-Fi"), &on, conf_kind("wifi"));
    kui_close(ui);
}

/* conformance::build_table (ADR 0033): a KUI_TABLE column of a fit header
 * row and three grow rows, each a bare text, a fixed box and a grow box,
 * so the label column is its longest label, the fixed column its widest
 * cell and the grow column takes the rest. Mirrors conformance::TABLE_ROWS. */
static void conf_table(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    static const char *labels[3] = {"abc", "abcde", "ab"};
    static const float rows[3][2] = {{30, 10}, {50, 12}, {20, 8}};
    KuiTextStyle style = {.size = 12};
    KuiSpec table = {
        .dir = KUI_TABLE,
        .pad_l = 4, .pad_r = 4, .pad_t = 4, .pad_b = 4,
        .gap = 2,
        .width = {KUI_FIXED, 200}, .bg = 0x101018ff,
    };
    kui_open_keyed(ui, KUI_STR("table"), &table, NULL);
    KuiSpec header = {.dir = KUI_ROW, .gap = 6};
    kui_open(ui, &header, NULL);
    kui_text(ui, KUI_STR("name"), &style);
    kui_text(ui, KUI_STR("w"), &style);
    kui_close(ui);
    for (int i = 0; i < 3; i++) {
        KuiSpec row = {.dir = KUI_ROW, .gap = 6, .width = {KUI_GROW, 1}};
        kui_open(ui, &row, NULL);
        kui_text(ui, (KuiStr){labels[i], strlen(labels[i])}, &style);
        KuiSpec fixed = {.width = {KUI_FIXED, rows[i][0]},
                         .height = {KUI_FIXED, rows[i][1]},
                         .bg = 0x30344aff};
        kui_open(ui, &fixed, NULL);
        kui_close(ui);
        KuiSpec grow = {.width = {KUI_GROW, 1},
                        .height = {KUI_FIXED, rows[i][1]},
                        .bg = 0x3b5bd4ff};
        kui_open(ui, &grow, NULL);
        kui_close(ui);
        kui_close(ui);
    }
    kui_close(ui);
}

/* An i3-style tab bar twice: grow tabs with KUI_MIN_FIT as their floor,
 * in a bar with room (two split it evenly) and in one without (four sit at
 * their labels' widths and the bar scrolls x). Mirrors conformance::TAB_*. */
static void conf_tabs(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    static const float roomy[2][2] = {{30, 12}, {50, 8}};
    static const float crowded[4] = {60, 70, 80, 90};
    KuiSpec outer = {.pad_l = 4, .pad_r = 4, .pad_t = 4, .pad_b = 4, .gap = 4};
    kui_open(ui, &outer, NULL);
    KuiSpec bar = {.dir = KUI_ROW, .width = {KUI_FIXED, 200},
                   .height = {KUI_FIXED, 20}, .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR("roomy"), &bar, NULL);
    for (int i = 0; i < 2; i++) {
        KuiSpec tab = {.width = {KUI_GROW, 1}, .min_w = KUI_MIN_FIT,
                       .height = {KUI_PERCENT, 0.5f}, .min_h = KUI_MIN_FIT,
                       .bg = 0x30344aff};
        kui_open(ui, &tab, NULL);
        KuiSpec label = {.width = {KUI_FIXED, roomy[i][0]},
                         .height = {KUI_FIXED, roomy[i][1]},
                         .bg = 0x3b5bd4ff};
        kui_open(ui, &label, NULL);
        kui_close(ui);
        kui_close(ui);
    }
    kui_close(ui);
    bar.overflow = KUI_SCROLL_X;
    kui_open_keyed(ui, KUI_STR("crowded"), &bar, NULL);
    for (int i = 0; i < 4; i++) {
        KuiSpec tab = {.width = {KUI_GROW, 1}, .min_w = KUI_MIN_FIT,
                       .height = {KUI_GROW, 1}, .bg = 0x30344aff};
        kui_open(ui, &tab, NULL);
        KuiSpec label = {.width = {KUI_FIXED, crowded[i]},
                         .height = {KUI_FIXED, 12}, .bg = 0x3b5bd4ff};
        kui_open(ui, &label, NULL);
        kui_close(ui);
        kui_close(ui);
    }
    kui_close(ui);
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

/* conformance::build_scrollbar: the four scrollbar rows on three scrollers
 * of the same list - hidden, styled (an 8 px thumb in two colours), auto. */
static void conf_scrollbar_list(KuiCtx *ui, const char *key, const KuiSpec *list) {
    kui_open_keyed(ui, KUI_STR(key), list, NULL);
    KuiSpec item = {.width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 20},
                    .bg = 0x30344aff};
    for (int i = 0; i < 6; i++) {
        char name[8];
        snprintf(name, sizeof name, "i%d", i);
        kui_open_keyed(ui, KUI_STR(name), &item, NULL);
        kui_close(ui);
    }
    kui_close(ui);
}

static void conf_scrollbar(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.dir = KUI_ROW, .pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10,
                     .gap = 10};
    kui_open(ui, &outer, NULL);
    KuiSpec hidden = {.width = {KUI_FIXED, 90}, .height = {KUI_FIXED, 60}, .gap = 0,
                      .overflow = KUI_SCROLL_Y, .bg = 0x101018ff,
                      .scrollbar = KUI_SCROLLBAR_HIDDEN};
    conf_scrollbar_list(ui, "hidden", &hidden);
    KuiSpec styled = {.width = {KUI_FIXED, 90}, .height = {KUI_FIXED, 60}, .gap = 0,
                      .overflow = KUI_SCROLL_Y, .bg = 0x101018ff,
                      .scrollbar_width = 8, .scrollbar_color = 0x3b5bd4ff,
                      .scrollbar_active_color = 0xffcc00ff};
    conf_scrollbar_list(ui, "styled", &styled);
    KuiSpec autobar = {.width = {KUI_FIXED, 90}, .height = {KUI_FIXED, 60}, .gap = 0,
                       .overflow = KUI_SCROLL_Y, .bg = 0x101018ff,
                       .scrollbar = KUI_SCROLLBAR_AUTO};
    conf_scrollbar_list(ui, "auto", &autobar);
    kui_close(ui);
}

/* conformance::build_tokens (ADR 0027): the table declared every build —
 * with `surface` in it, refused as `reserved-token` — then read back by
 * name, since a C prop carries no reference: `kui_token_color` answers a
 * declared name and a theme role's, false for `nothing` (and raises
 * `unknown-token`), so the value written is what the other adapters'
 * `$name` resolves to. */
static void conf_tokens(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiColorToken colors[] = {
        {KUI_STR("peach"), 0xffcc99ffu, 0xffcc99ffu},
        {KUI_STR("ink"), 0x202020ffu, 0xe0e0e0ffu},
        {KUI_STR("surface"), 0xff0000ffu, 0xff0000ffu},
    };
    KuiLengthToken lengths[] = {
        {KUI_STR("side_w"), 60.0f}, {KUI_STR("gap"), 8.0f}, {KUI_STR("big"), 16.0f},
    };
    kui_tokens_set(ui, colors, 3, lengths, 3);
    /* conformance::TOKEN_DERIVED (ADR 0028): the same six, `bad` dropped by
     * the core with `unknown-token`; `up` derives from `surface`, which the
     * declaration above lost to the role. */
    KuiColorOp lit_ops[] = {{KUI_OP_LIFT, 0.3f, {0}}};
    KuiColorOp dim_ops[] = {{KUI_OP_MIX, 0.5f, KUI_STR("peach")}, {KUI_OP_DARKEN, 0.5f, {0}}};
    KuiColorOp up_ops[] = {{KUI_OP_RAISE, 0.25f, {0}}};
    KuiColorOp deep_ops[] = {{KUI_OP_ALPHA, 0.5f, {0}}};
    KuiColorOp read_ops[] = {{KUI_OP_READABLE, 4.5f, KUI_STR("ink")}};
    KuiColorOp bad_ops[] = {{KUI_OP_LIFT, 0.1f, {0}}};
    KuiDerivedToken derived[] = {
        {KUI_STR("lit"), KUI_STR("peach"), lit_ops, 1},
        {KUI_STR("dim"), KUI_STR("ink"), dim_ops, 2},
        {KUI_STR("up"), KUI_STR("surface"), up_ops, 1},
        {KUI_STR("deep"), KUI_STR("lit"), deep_ops, 1},
        {KUI_STR("read"), KUI_STR("peach"), read_ops, 1},
        {KUI_STR("bad"), KUI_STR("nothing"), bad_ops, 1},
    };
    kui_tokens_derive(ui, derived, 6);
    uint32_t peach = 0, ink = 0, surface = 0, nothing = 0;
    float side_w = 0, gap = 0, big = 0, radius = 0;
    kui_token_color(ui, KUI_STR("peach"), &peach);
    kui_token_color(ui, KUI_STR("ink"), &ink);
    kui_token_color(ui, KUI_STR("surface"), &surface);
    kui_token_color(ui, KUI_STR("nothing"), &nothing);
    kui_token_length(ui, KUI_STR("side_w"), &side_w);
    kui_token_length(ui, KUI_STR("gap"), &gap);
    kui_token_length(ui, KUI_STR("big"), &big);
    kui_token_length(ui, KUI_STR("radius"), &radius);
    KuiSpec outer = {.dir = KUI_ROW, .pad_l = gap, .pad_r = 10, .pad_t = 10, .pad_b = 10,
                     .gap = gap};
    kui_open(ui, &outer, NULL);
    KuiSpec cell = {.width = {KUI_FIXED, side_w}, .height = {KUI_FIXED, 30}};
    cell.bg = peach;
    kui_open_keyed(ui, KUI_STR("peach"), &cell, NULL);
    kui_close(ui);
    cell.bg = ink;
    cell.border_w = gap;
    cell.border_color = peach;
    kui_open_keyed(ui, KUI_STR("ink"), &cell, NULL);
    kui_close(ui);
    cell.border_w = 0;
    cell.border_color = 0;
    cell.bg = surface;
    cell.radius = radius;
    kui_open_keyed(ui, KUI_STR("role"), &cell, NULL);
    kui_close(ui);
    cell.radius = 0;
    cell.bg = nothing;
    kui_open_keyed(ui, KUI_STR("missing"), &cell, NULL);
    kui_close(ui);
    static const char *const derived_keys[] = {"lit", "dim", "up", "deep", "read"};
    for (size_t i = 0; i < 5; i++) {
        uint32_t c = 0;
        KuiStr key = {(const uint8_t *)derived_keys[i], strlen(derived_keys[i])};
        kui_token_color(ui, key, &c);
        cell.bg = c;
        kui_open_keyed(ui, key, &cell, NULL);
        kui_close(ui);
    }
    KuiSpan spans[] = {
        {.text = KUI_STR("tokens")},
        {.text = KUI_STR("x"), .color = ink},
    };
    KuiTextStyle style = {.size = big, .color = peach};
    kui_rich_text(ui, spans, 2, &style);
    kui_close(ui);
}

/* Scroll anchoring (backlog C26 step 3): two scrollers of the same rows,
 * one with `anchor`; phase 1 prepends a taller row to both. */
static void conf_anchor(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    KuiSpec outer = {.dir = KUI_ROW, .pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10,
                     .gap = 10};
    kui_open(ui, &outer, NULL);
    const char *keys[2] = {"anchored", "plain"};
    for (int n = 0; n < 2; n++) {
        KuiSpec list = {.width = {KUI_FIXED, 90}, .height = {KUI_FIXED, 60},
                        .overflow = KUI_SCROLL_Y, .bg = 0x101018ff, .anchor = n == 0};
        kui_open_keyed(ui, KUI_STR(keys[n]), &list, NULL);
        if (phase >= 1) {
            KuiSpec tall = {.width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 30},
                            .bg = 0x30344aff};
            kui_open_keyed(ui, KUI_STR("new"), &tall, NULL);
            kui_close(ui);
        }
        KuiSpec item = {.width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 20},
                        .bg = 0x30344aff};
        for (int i = 0; i < 6; i++) {
            char name[8];
            snprintf(name, sizeof name, "i%d", i);
            kui_open_keyed(ui, KUI_STR(name), &item, NULL);
            kui_close(ui);
        }
        kui_close(ui);
    }
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

/* One of conf_clip_float's two nodes: a parent-anchored float at (dx, -20)
 * on the canvas that posts `key` when clicked, cut by the canvas's clip
 * when `clip` is set (float_clip, ABI 19). */
static void conf_clip_float_node(KuiCtx *ui, const char *key, float dx, uint32_t clip,
                                 uint32_t bg, const char *label) {
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR(key)));
    KuiSpec spec = {
        .float_mode = KUI_FLOAT_PARENT,
        .float_dx = dx, .float_dy = -20, .float_clip = clip,
        .width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 40}, .bg = bg,
        .label = KUI_STR(label),
    };
    kui_open_keyed(ui, KUI_STR(key), &spec, tag);
    kui_close(ui);
}

/* conformance::build_clip_float (backlog F90): a toolbar over a clip
 * canvas, and two nodes on the canvas panned half past its top edge -
 * the first cut there, the second escaping as every float did. */
static void conf_clip_float(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}};
    kui_open(ui, &outer, NULL);
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR("toolbar")));
    KuiSpec toolbar = {.dir = KUI_ROW, .width = {KUI_GROW, 1}, .height = {KUI_FIXED, 40},
                       .bg = 0x3a3f52ff, .label = KUI_STR("Toolbar")};
    kui_open_keyed(ui, KUI_STR("toolbar"), &toolbar, tag);
    kui_close(ui);
    KuiSpec canvas = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
                      .overflow = KUI_CLIP, .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR("canvas"), &canvas, NULL);
    conf_clip_float_node(ui, "node", 40, 1, 0x3b5bd4ff, "Node");
    conf_clip_float_node(ui, "free", 160, 0, 0x73d98cff, "Free");
    kui_close(ui);
    kui_close(ui);
}

/* conformance::build_pixel_snap: a row of three boxes at 40.5 by 20.25,
 * the first two painted on whole pixels (pixel_snap, ABI 20) and the first
 * with a hard shadow, the third drawn where layout put it. */
static void conf_pixel_snap(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec row = {.dir = KUI_ROW};
    kui_open(ui, &row, NULL);
    KuiSpec first = {.width = {KUI_FIXED, 40.5f}, .height = {KUI_FIXED, 20.25f},
                     .bg = 0xd9738cff, .pixel_snap = 1, .shadow_color = 0x000000ff};
    kui_open(ui, &first, NULL);
    kui_close(ui);
    KuiSpec second = {.width = {KUI_FIXED, 40.5f}, .height = {KUI_FIXED, 20.25f},
                      .bg = 0x73d98cff, .pixel_snap = 1};
    kui_open(ui, &second, NULL);
    kui_close(ui);
    KuiSpec third = {.width = {KUI_FIXED, 40.5f}, .height = {KUI_FIXED, 20.25f},
                     .bg = 0x3b5bd4ff};
    kui_open(ui, &third, NULL);
    kui_close(ui);
    kui_close(ui);
}

/* A fixed box that posts `key` when clicked and is named `label`: the
 * nodes and rows of conf_clip_access. */
static KuiSpec conf_clip_access_button(float w, float h, uint32_t bg, const char *label) {
    KuiSpec spec = {.width = {KUI_FIXED, w}, .height = {KUI_FIXED, h}, .bg = bg,
                    .label = KUI_STR(label)};
    return spec;
}

/* conformance::build_clip_access (backlog F93): access rects cut to the
 * clip. A toolbar over a clip canvas beside a short scroller; on the
 * canvas a float straddling its top, one wholly past it (both
 * float_clip) and one escaping, and in the scroller three rows, the
 * second half out and the third wholly out. */
static void conf_clip_access(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}};
    kui_open(ui, &outer, NULL);
    KuiSpec toolbar = conf_clip_access_button(0, 40, 0x3a3f52ff, "Toolbar");
    toolbar.width = (KuiSizing){KUI_GROW, 1};
    kui_open_keyed(ui, KUI_STR("toolbar"), &toolbar, conf_kind("toolbar"));
    kui_close(ui);
    KuiSpec body = {.dir = KUI_ROW, .width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}};
    kui_open(ui, &body, NULL);
    KuiSpec canvas = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
                      .overflow = KUI_CLIP, .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR("canvas"), &canvas, NULL);
    const struct { const char *key, *label; float dx, dy; uint32_t clip; } nodes[] = {
        {"cut", "Cut", 20, -20, 1},
        {"past", "Past", 100, -60, 1},
        {"free", "Free", 140, -60, 0},
    };
    for (size_t i = 0; i < sizeof nodes / sizeof nodes[0]; i++) {
        KuiSpec node = conf_clip_access_button(60, 40, 0x3b5bd4ff, nodes[i].label);
        node.float_mode = KUI_FLOAT_PARENT;
        node.float_dx = nodes[i].dx;
        node.float_dy = nodes[i].dy;
        node.float_clip = nodes[i].clip;
        kui_open_keyed(ui, KUI_STR(nodes[i].key), &node, conf_kind(nodes[i].key));
        kui_close(ui);
    }
    kui_close(ui);
    KuiSpec list = {.width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 50},
                    .overflow = KUI_SCROLL_Y, .bg = 0x202030ff};
    kui_open_keyed(ui, KUI_STR("list"), &list, NULL);
    const char *rows[][2] = {{"row0", "Row 0"}, {"row1", "Row 1"}, {"row2", "Row 2"}};
    for (size_t i = 0; i < 3; i++) {
        KuiSpec row = conf_clip_access_button(100, 30, 0x73d98cff, rows[i][1]);
        kui_open_keyed(ui, KUI_STR(rows[i][0]), &row, conf_kind(rows[i][0]));
        kui_close(ui);
    }
    kui_close(ui);
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
    /* KuiSpec.description is the same slot without the hover tracking or
     * the float: spoken, never drawn. */
    KuiSpec saved = {.dir = KUI_ROW, .width = {KUI_FIXED, 100},
                     .height = {KUI_FIXED, 20}, .role = KUI_ROLE_BUTTON,
                     .label = KUI_STR("Save"),
                     .description = KUI_STR("Nothing to save yet")};
    kui_open(ui, &saved, NULL);
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
    /* The other root declaration with no node: the frame asks for the
     * window on top, and the report says it asked. */
    kui_set_always_on_top(ui, true);
    /* And the third: secure keyboard entry (backlog F85). */
    kui_set_secure_input(ui, true);
    KuiSpec outer = {.gap = 6};
    kui_open(ui, &outer, NULL);
    /* kui_titlebar_with appends its own cluster after the body; the second
     * one is kui_window_buttons called directly, in a strip laid out here -
     * the "fully custom titlebar" the element exists for. */
    kui_titlebar_with(ui, conf_titlebar_body, NULL);
    KuiSpec strip = {.dir = KUI_ROW, .width = {KUI_GROW, 1}, .keep_focus = 1};
    kui_open(ui, &strip, NULL);
    kui_window_buttons(ui);
    kui_close(ui);
    KuiValue *sink_tag = kui_value_map();
    kui_value_map_set(sink_tag, KUI_STR("kind"), kui_value_str(KUI_STR("sink")));
    KuiSpec sink = {.width = {KUI_FIXED, 40}, .height = {KUI_FIXED, 16},
                    .bg = 0x22242cff, .focusable = 1, .label = KUI_STR("Sink"),
                    .on_focus = sink_tag};
    uint64_t key = kui_open_keyed(ui, KUI_STR("sink"), &sink, NULL);
    kui_value_free(sink_tag);
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
    KuiSpec go_rows = {.description = KUI_STR("Starts the run")};
    kui_button_with(ui, KUI_STR("go"), &go_rows, go);
    /* Every row the stock button admits, on a button the steps never
     * touch: the name past its text, disabled, and a tooltip whose
     * description reaches the access row while nothing hovers it. */
    KuiValue *stop = kui_value_map();
    kui_value_map_set(stop, KUI_STR("kind"), kui_value_str(KUI_STR("stop")));
    KuiSpec stop_rows = {.label = KUI_STR("Stop the run"),
                         .disabled = 1,
                         .tooltip = KUI_STR("Nothing is running")};
    kui_button_with(ui, KUI_STR("stop"), &stop_rows, stop);
    /* A field with the wrap row declared (backlog F44): KUI_EDIT_WRAP is
     * the declaration, the style's wrap the mode - its zero is WORD, so
     * the style alone could not say. The seed folds onto two lines. */
    KuiTextStyle s13 = {.size = 13};
    KuiSpec note = {.width = {KUI_FIXED, 160}, .label = KUI_STR("Note")};
    kui_text_edit(ui, KUI_STR("note"), KUI_STR("hello, on two lines in a narrow field"), &s13,
                  KUI_EDIT_WRAP, &note);
    /* A slider that names its own reading: value_text is what a reader says
     * instead of the percentage value_now and the range alone would give,
     * and it comes back in KuiAccessNode.value (backlog F8). */
    KuiSpec focus = {.width = {KUI_FIXED, 120},
                     .height = {KUI_FIXED, 12},
                     .role = KUI_ROLE_SLIDER,
                     .label = KUI_STR("Focus length"),
                     .value_set = KUI_VALUE_NOW | KUI_VALUE_MIN | KUI_VALUE_MAX,
                     .value_now = 25,
                     .value_min = 5,
                     .value_max = 60,
                     .value_text = KUI_STR("25 minutes")};
    kui_open_keyed(ui, KUI_STR("focus"), &focus, NULL);
    kui_close(ui);
    kui_close(ui);
    kui_value_free(menu);
}

static void conf_media(KuiCtx *ui, const Fixtures *f, int phase) {
    KuiSpec outer = {.pad_l = 6, .pad_r = 6, .pad_t = 6, .pad_b = 6, .gap = 4};
    kui_open(ui, &outer, NULL);
    KuiSpec img = {.width = {KUI_FIXED, 16}, .radius = 2};
    kui_image(ui, f->image, &img);
    /* ADR 0025: the icon as `contain` in a box twice its aspect, then the
     * stream fixture plain, `nearest`, and `cover` in a square box. */
    KuiSpec icon = {.width = {KUI_FIXED, 32}, .height = {KUI_FIXED, 16},
                    .label = KUI_STR("Icon")};
    kui_image_with(ui, f->image, KUI_SAMPLING_LINEAR, KUI_FIT_CONTAIN, &icon);
    KuiSpec stream = {.width = {KUI_FIXED, 16}, .label = KUI_STR("Stream")};
    kui_image(ui, f->stream, &stream);
    KuiSpec crisp = {.width = {KUI_FIXED, 16}, .label = KUI_STR("Crisp")};
    kui_image_with(ui, f->stream, KUI_SAMPLING_NEAREST, KUI_FIT_FILL, &crisp);
    KuiSpec cropped = {.width = {KUI_FIXED, 12}, .height = {KUI_FIXED, 12},
                       .label = KUI_STR("Cropped")};
    kui_image_with(ui, f->stream, KUI_SAMPLING_LINEAR, KUI_FIT_COVER, &cropped);
    KuiAudio music = {.src = f->sound, .volume = 0.5f, .looped = 1};
    kui_audio(ui, KUI_STR("music"), &music, NULL);
    kui_latency_graph(ui);
    /* The two phase 1 drops: `chime` asked to finish, so its removal
     * releases the playback and queues no stop; `blip` did not. */
    if (phase == 0) {
        KuiAudio chime = KUI_AUDIO_INIT(f->sound);
        chime.finish = 1;
        kui_audio(ui, KUI_STR("chime"), &chime, NULL);
        KuiAudio blip = KUI_AUDIO_INIT(f->sound);
        kui_audio(ui, KUI_STR("blip"), &blip, NULL);
    }
    kui_close(ui);
}

/* docs/adr/0025-the-image-is-the-canvas.md, decision 6: five fills in a
 * 200x120 canvas - the triangle declares the one input prop the spec
 * carries, which a polygon ignores; the star is keyed; the nine-point
 * outline loses its ninth with a warning; the quad is faded. The vertices
 * are conformance::POLYGON_* to the number. */
static void conf_polygon(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec canvas = {.width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 120},
                      .bg = 0x14161eff};
    kui_open(ui, &canvas, NULL);
    float tri[] = {10, 10, 60, 20, 20, 50};
    KuiSpec blue = {.bg = 0x7f9cf5ff, .label = KUI_STR("Triangle")};
    KuiValue *tri_click = kui_value_map();
    kui_value_map_set(tri_click, KUI_STR("kind"), kui_value_str(KUI_STR("tri")));
    kui_polygon(ui, KUI_STR(""), tri, 3, &blue, tri_click, NULL, NULL);
    float arrow[] = {80, 10, 130, 30, 80, 50, 95, 30};
    KuiSpec orange = {.bg = 0xd8863bff};
    kui_polygon(ui, KUI_STR(""), arrow, 4, &orange, NULL, NULL, NULL);
    float star[] = {170, 10, 176, 24, 190, 30, 176, 36, 170, 50, 164, 36, 150, 30, 164, 24};
    KuiSpec yellow = {.bg = 0xf5d67fff};
    kui_polygon(ui, KUI_STR("star"), star, 8, &yellow, NULL, NULL, NULL);
    float nine[] = {10, 70, 30, 65, 50, 70, 70, 65, 90, 70, 90, 110, 50, 100, 10, 110, 5, 90};
    KuiSpec green = {.bg = 0x9ad9a0ff};
    kui_polygon(ui, KUI_STR(""), nine, 9, &green, NULL, NULL, NULL);
    float quad[] = {110, 70, 190, 70, 180, 110, 120, 110};
    KuiSpec pink = {.bg = 0xe07a8aff, .opacity_set = 1, .opacity = 0.5f};
    kui_polygon(ui, KUI_STR(""), quad, 4, &pink, NULL, NULL, NULL);
    kui_close(ui);
}

/* docs/adr/0010-a-segment-primitive.md: three strokes and a box in a 200x120
 * canvas; the elbow's on_click is the one a line ignores. The curve is keyed
 * through kui_polyline's label; the other two are auto-keyed. */
/* docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md: four
 * fragments - plain, keyed with a child painted over it, a dead handle that
 * draws nothing, and one with eighteen params so the warning fires - then
 * the image input (backlog V1): the sampling fixture over the atlas-backed
 * icon, over the texture-backed stream, and over an image that is live
 * nowhere, which draws nothing. */
static void conf_fragments(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)phase;
    KuiSpec canvas = {.width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 120},
                      .gap = 4, .bg = 0x14161eff};
    kui_open(ui, &canvas, NULL);

    KuiSpec plain = {.width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 40}};
    kui_fragment(ui, f->fragment, CONF_FRAGMENT_PARAMS, 16, &plain);

    KuiSpec card = {.width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 40},
                    .pad_l = 6, .pad_r = 6, .pad_t = 6, .pad_b = 6,
                    .radius = 8, .opacity_set = 1, .opacity = 0.5f};
    kui_fragment_open(ui, KUI_STR("card"), f->fragment, CONF_FRAGMENT_PARAMS, 16, &card);
    KuiSpec child = {.width = {KUI_FIXED, 20}, .height = {KUI_FIXED, 10}, .bg = 0x202030ff};
    kui_open(ui, &child, NULL);
    kui_close(ui);
    kui_close(ui);

    KuiSpec dead = {.width = {KUI_FIXED, 20}, .height = {KUI_FIXED, 10}};
    kui_fragment(ui, 0, CONF_FRAGMENT_PARAMS, 16, &dead);

    KuiSpec longp = {.width = {KUI_FIXED, 30}, .height = {KUI_FIXED, 12}};
    kui_fragment(ui, f->fragment, CONF_FRAGMENT_PARAMS_LONG, 18, &longp);

    KuiSpec row = {.dir = KUI_ROW, .gap = 4};
    kui_open(ui, &row, NULL);
    KuiSpec sq = {.width = {KUI_FIXED, 24}, .height = {KUI_FIXED, 24}};
    kui_fragment_with(ui, KUI_STR(""), f->sampler, f->image, CONF_FRAGMENT_IMAGE_PARAMS, 4, &sq);
    KuiSpec wide = {.width = {KUI_FIXED, 32}, .height = {KUI_FIXED, 8}};
    kui_fragment_with(ui, KUI_STR(""), f->sampler, f->stream, CONF_FRAGMENT_IMAGE_PARAMS, 4, &wide);
    /* An image handle live in no session: the fragment draws nothing. */
    kui_fragment_with(ui, KUI_STR(""), f->sampler, f->dead, CONF_FRAGMENT_IMAGE_PARAMS, 4, &sq);
    kui_close(ui);

    kui_close(ui);
}

/* Backlog K4: a wave in red under a span, a green solid line through the
 * style's fields, dots in their own colour, and an undercurl over three
 * cells carrying KUI_CELL_WAVY and a red `ul`. */
static void conf_underlines(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10, .gap = 4};
    kui_open(ui, &outer, NULL);
    KuiTextStyle mono = {.size = 14, .family = KUI_FONT_MONO, .line_height = 20};
    KuiSpan spans[] = {
        {.text = KUI_STR("let ")},
        {.text = KUI_STR("value"), .underline_color = 0xff0000ff,
         .underline_style = KUI_UNDERLINE_WAVY},
    };
    kui_rich_text(ui, spans, 2, &mono);
    KuiTextStyle green = mono;
    green.underline_color = 0x00ff00ff;
    kui_text(ui, KUI_STR("warn"), &green);
    KuiTextStyle dotted = mono;
    dotted.underline_color = 0x7f9cf5ff;
    dotted.underline_style = KUI_UNDERLINE_DOTTED;
    kui_text(ui, KUI_STR("dots"), &dotted);
    KuiCell screen[3] = {0};
    for (int i = 0; i < 3; i++) {
        screen[i].ch = (uint32_t)"abc"[i];
        screen[i].fg = 0xd6d8e0ff;
        screen[i].flags = KUI_CELL_WAVY;
        screen[i].ul = 0xff0000ff;
    }
    KuiSpec term = {.label = KUI_STR("term")};
    kui_cells(ui, KUI_STR("term"), 1, 3, screen, 3, &mono, &term, NULL, NULL, NULL, 0, 0, 0, 0,
              0);
    kui_close(ui);
}

/* conformance::build_joined_backgrounds (backlog F101): four texts of mono
 * spans, the first three's selected spans in one translucent colour with
 * bg_radius 4 (ABI 20), joined into one shape; the fourth in another. */
static void conf_joined_backgrounds(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10};
    kui_open(ui, &outer, NULL);
    KuiTextStyle mono = {.size = 14, .family = KUI_FONT_MONO, .line_height = 20};
    const uint32_t sel = 0x3b5bd466;
    KuiSpan first[] = {
        {.text = KUI_STR("let ")},
        {.text = KUI_STR("a = 1;"), .bg = sel, .bg_radius = 4},
    };
    kui_rich_text(ui, first, 2, &mono);
    KuiSpan second[] = {{.text = KUI_STR("let b = 22;"), .bg = sel, .bg_radius = 4}};
    kui_rich_text(ui, second, 1, &mono);
    KuiSpan third[] = {
        {.text = KUI_STR("c"), .bg = sel, .bg_radius = 4},
        {.text = KUI_STR(" + d")},
    };
    kui_rich_text(ui, third, 2, &mono);
    KuiSpan fourth[] = {{.text = KUI_STR("find"), .bg = 0xd9738c66, .bg_radius = 4}};
    kui_rich_text(ui, fourth, 1, &mono);
    kui_close(ui);
}

static void conf_lines(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec canvas = {.width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 120},
                      .bg = 0x14161eff};
    kui_open(ui, &canvas, NULL);
    kui_line(ui, 10, 10, 90, 70, 2, 0x7f9cf5ff, NULL);
    float elbow[] = {100, 20, 140, 20, 140, 60};
    /* The elbow takes a click, and is hit by its stroke (ADR 0026). */
    KuiSpec elbow_spec = {.label = KUI_STR("Elbow")};
    KuiValue *elbow_click = kui_value_map();
    kui_value_map_set(elbow_click, KUI_STR("kind"), kui_value_str(KUI_STR("elbow")));
    kui_polyline(ui, KUI_STR(""), elbow, 3, 3, 0xd8863bff, false, &elbow_spec, elbow_click,
                 NULL, NULL);
    float curve[] = {20, 100, 60, 80, 100, 110, 180, 90};
    KuiSpec faded = {.opacity_set = 1, .opacity = 0.5f};
    kui_polyline(ui, KUI_STR("curve"), curve, 4, 1.5f, 0x9ad9a0ff, true, &faded, NULL, NULL,
                 NULL);
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
 * integer tag, so the report's event column shows the phase. Left open, so
 * the third can hold a button (see conf_keys). */
/* UTF-8 for one scalar value, for the steps that may carry any. */
static size_t conf_utf8(uint32_t c, uint8_t out[4]) {
    if (c < 0x80) { out[0] = (uint8_t)c; return 1; }
    if (c < 0x800) { out[0] = 0xc0 | (c >> 6); out[1] = 0x80 | (c & 0x3f); return 2; }
    if (c < 0x10000) {
        out[0] = 0xe0 | (c >> 12); out[1] = 0x80 | ((c >> 6) & 0x3f); out[2] = 0x80 | (c & 0x3f);
        return 3;
    }
    out[0] = 0xf0 | (c >> 18); out[1] = 0x80 | ((c >> 12) & 0x3f);
    out[2] = 0x80 | ((c >> 6) & 0x3f); out[3] = 0x80 | (c & 0x3f);
    return 4;
}

/* An IME against both kinds of editor (backlog C17): the custom one is a
 * sink holding a line with a caret, the stock one kui_text_edit. */
static void conf_ime(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10, .gap = 6};
    kui_open(ui, &outer, NULL);
    KuiValue *ed = kui_value_map();
    kui_value_map_set(ed, KUI_STR("kind"), kui_value_str(KUI_STR("ed")));
    KuiSpec buf = {.width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 24}, .bg = 0x1b1d27ff,
                   .role = KUI_ROLE_MULTILINE_TEXT_INPUT, .label = KUI_STR("Buffer")};
    kui_open_with(ui, KUI_STR("buffer"), &buf, NULL, NULL, ed, NULL);
    KuiSpec l0 = {.dir = KUI_ROW, .height = {KUI_FIXED, 20}, .role = KUI_ROLE_LINE,
                  .value_set = KUI_VALUE_CARET, .caret = 1};
    kui_open_keyed(ui, KUI_STR("l0"), &l0, NULL);
    KuiTextStyle mono = {.size = 13, .family = KUI_FONT_MONO};
    kui_text(ui, KUI_STR("ab"), &mono);
    kui_close(ui);
    kui_close(ui);
    KuiTextStyle s13 = {.size = 13};
    KuiSpec note = {.width = {KUI_FIXED, 200}, .label = KUI_STR("Note")};
    kui_text_edit(ui, KUI_STR("note"), KUI_STR(""), &s13, 0, &note);
    kui_close(ui);
}

/* A terminal's screen as one node (backlog C20): the cells travel as a
 * KuiCell array, and the click on the fourth cell names it. */
static void conf_cells(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10};
    kui_open(ui, &outer, NULL);
    KuiCell screen[11] = {0};
    const char *txt = "hello world";
    for (int i = 0; i < 11; i++) {
        screen[i].ch = (uint32_t)txt[i];
        screen[i].fg = 0xd6d8e0ff;
        screen[i].bg = i < 3 ? 0x1a1d27ff : 0;
    }
    KuiTextStyle style = {.size = 13, .family = KUI_FONT_MONO, .line_height = 18};
    KuiValue *hit = kui_value_map();
    kui_value_map_set(hit, KUI_STR("kind"), kui_value_str(KUI_STR("hit")));
    KuiSpec term = {.label = KUI_STR("term")};
    kui_cells(ui, KUI_STR("term"), 1, 11, screen, 11, &style, &term, hit, NULL, NULL, 0, 3,
              KUI_CELL_CURSOR_BLOCK, 0x6a8bffff, 0);
    kui_close(ui);
}

static void conf_keys_sink(KuiCtx *ui, const char *name, uint32_t key_up) {
    KuiSpec spec = {.dir = KUI_ROW, .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 24},
                    .bg = 0x1b1d27ff, .role = KUI_ROLE_GROUP, .label = KUI_STR(name),
                    .key_up = key_up};
    kui_open_with(ui, KUI_STR(name), &spec, NULL, NULL, kui_value_int(1), NULL);
}

static void conf_keys(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10, .gap = 6};
    kui_open(ui, &outer, NULL);
    conf_keys_sink(ui, "press", 0);
    kui_close(ui);
    conf_keys_sink(ui, "held", 1);
    kui_close(ui);
    /* A shell over a ring: the sink hears what the button inside it does
     * not claim, and the button holds focus from the first frame
     * (docs/adr/0011-keys-bubble-to-the-enclosing-sink.md). */
    conf_keys_sink(ui, "shell", 1);
    KuiValue *go_tag = kui_value_map();
    kui_value_map_set(go_tag, KUI_STR("kind"), kui_value_str(KUI_STR("go")));
    KuiSpec go = {.dir = KUI_ROW, .width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 16},
                  .bg = 0x3b5bd4ff, .label = KUI_STR("Go")};
    uint64_t go_key = kui_open_keyed(ui, KUI_STR("go"), &go, go_tag);
    kui_close(ui);
    kui_set_key_focus(ui, go_key);
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
/* conformance::EXIT_ROWS: more one-node subtrees than the budget, dropped
 * in one frame and refused whole (docs/adr/0012-the-exit-budget.md). */
#define CONF_EXIT_ROWS 600

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

/* Exit transitions (docs/adr/0005-the-paint-vocabulary.md). Three subtrees
 * the view stops declaring in phase 1 - one still in flight at the end, one
 * already over, one that comes back in phase 2 - then one past the budget
 * in phase 3 and 600 one-node rows in phase 4, each frame refused whole
 * (docs/adr/0012-the-exit-budget.md) - and two that never leave. */
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

    /* More one-node departures than the budget, each a solid quad half a
     * pixel wide, in a slot that keeps its size when they go. */
    KuiSpec slot_rows = {.dir = KUI_ROW, .width = {KUI_FIXED, 300},
                         .height = {KUI_FIXED, 4}, .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR("slotRows"), &slot_rows, NULL);
    if (phase < 4) {
        KuiSpec cell = {.width = {KUI_FIXED, 0.5f}, .height = {KUI_FIXED, 4},
                        .bg = 0x8a8fa3ff, .transition_ms = 400,
                        .exit = {.set = KUI_ENTER_OPACITY, .opacity = 0}};
        for (int i = 0; i < CONF_EXIT_ROWS; i++) {
            kui_open(ui, &cell, NULL);
            kui_close(ui);
        }
    }
    kui_close(ui);

    /* Last, and sized by children that have no size: dropping it takes only
     * the trailing gap with it. */
    if (phase < 3) {
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

/* conformance::build_selection: a `selectable` card of three runs the
 * pointer drags across, leaving the three highlight quads the report pins.
 * The row is on the container; the labels inside declare nothing. */
static void conf_selection(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec card = {.width = {KUI_FIXED, 200},
                    .pad_l = 8, .pad_r = 8, .pad_t = 8, .pad_b = 8,
                    .gap = 4, .bg = 0x14161eff, .selectable = 1};
    kui_open_keyed(ui, KUI_STR("card"), &card, NULL);
    KuiTextStyle s13 = {.size = 13};
    kui_text(ui, KUI_STR("one"), &s13);
    kui_text(ui, KUI_STR("two"), &s13);
    kui_text(ui, KUI_STR("three"), &s13);
    kui_close(ui);
}

/* conformance::build_selection_scroll: the same card, forty px tall and
 * scrolling, over six runs - what a press held past its edge scrolls
 * (ADR 0029). */
static void conf_selection_scroll(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec card = {.width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 40},
                    .pad_l = 8, .pad_r = 8, .pad_t = 8, .pad_b = 8,
                    .gap = 4, .bg = 0x14161eff, .overflow = KUI_SCROLL_Y, .selectable = 1};
    kui_open_keyed(ui, KUI_STR("card"), &card, NULL);
    KuiTextStyle s13 = {.size = 13};
    const char *lines[6] = {"one", "two", "three", "four", "five", "six"};
    for (int i = 0; i < 6; i++) kui_text(ui, KUI_STR(lines[i]), &s13);
    kui_close(ui);
}

/* conformance::build_cells_scroll: the `cells` screen three rows tall,
 * `selectable` and hearing the wheel, row 0 at 100 plus the phase - the
 * phase being how the scene's view answers a `scroll` event (ADR 0029). */
static void conf_cells_scroll(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10};
    kui_open(ui, &outer, NULL);
    KuiCell screen[33] = {0};
    const char *rows[3] = {"hello world", "brave", "bye"};
    for (int i = 0; i < 33; i++) {
        screen[i].ch = ' ';
        screen[i].fg = 0xd6d8e0ff;
    }
    for (int r = 0; r < 3; r++)
        for (int c = 0; rows[r][c]; c++) screen[r * 11 + c].ch = (uint32_t)rows[r][c];
    KuiTextStyle style = {.size = 13, .family = KUI_FONT_MONO, .line_height = 18};
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR("term")));
    KuiSpec term = {.label = KUI_STR("term"), .selectable = 1, .on_scroll = tag};
    kui_cells(ui, KUI_STR("term"), 3, 11, screen, 33, &style, &term, NULL, NULL, NULL, 0, 0,
              0, 0, 100 + (uint64_t)phase);
    kui_value_free(tag);
    kui_close(ui);
}

/* A virtual list's three built rows, each opened at its *data* index rather
 * than at the position it occupies (kui_open_indexed), between the two
 * spacers that stand in for the rows nobody built. The indices are past what
 * auto-keying under five children could reach, so a build that ignored them
 * would report a different access tree for the same four quads. */
static void conf_virtual(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec list = {.width = {KUI_FIXED, 120}, .height = {KUI_FIXED, 60},
                    .gap = 0, .overflow = KUI_SCROLL_Y, .bg = 0x101018ff,
                    .role = KUI_ROLE_LIST, .label = KUI_STR("log")};
    kui_open_keyed(ui, KUI_STR("list"), &list, NULL);
    /* conformance::VIRTUAL_ROW_COUNT: how many rows the list has, built or not. */
    kui_row_count(ui, 109);
    KuiSpec lead = {.width = {KUI_GROW, 1}, .height = {KUI_FIXED, 20}};
    kui_open_keyed(ui, KUI_STR("lead"), &lead, NULL);
    kui_close(ui);
    for (uint64_t i = 100; i < 103; i++) {
        char name[16];
        snprintf(name, sizeof name, "row %llu", (unsigned long long)i);
        KuiSpec row = {.width = {KUI_GROW, 1}, .height = {KUI_FIXED, 20},
                       .bg = 0x30344aff, .role = KUI_ROLE_LIST_ITEM,
                       .label = KUI_STR(name)};
        kui_open_indexed(ui, i, &row, NULL);
        kui_close(ui);
    }
    KuiSpec tail = {.width = {KUI_GROW, 1}, .height = {KUI_FIXED, 100}};
    kui_open_keyed(ui, KUI_STR("tail"), &tail, NULL);
    kui_close(ui);
    kui_close(ui);
}

/* conformance::LAYERS_ROWS: enough rows to overflow the viewport, so the
 * page has a bar for the popover to cover. */
#define CONF_LAYERS_ROWS 16

/* One of the two floats of conf_layers: a viewport float at (x, y) that
 * posts `kind` when clicked. The payload is consumed by kui_open_keyed. */
static void conf_layers_float(KuiCtx *ui, const char *key, float x, float y,
                              uint32_t bg, const char *kind, const char *label) {
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR(kind)));
    KuiSpec spec = {
        .float_mode = KUI_FLOAT_VIEWPORT,
        .float_anchor_x = KUI_START, .float_anchor_y = KUI_START,
        .float_self_x = KUI_START, .float_self_y = KUI_START,
        .float_dx = x, .float_dy = y,
        .width = {KUI_FIXED, 120}, .height = {KUI_FIXED, 80}, .bg = bg,
        .label = KUI_STR(label),
    };
    kui_open_keyed(ui, KUI_STR(key), &spec, tag);
    kui_close(ui);
}

/* conformance::build_layers: two floats over a scroller's bar (ADR 0023).
 * The toast is later in the tree than the popover, so it is over it in
 * phase 0; the popover closes in phase 1 and reopens in phase 2, which
 * puts it over the toast. */
static void conf_layers(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    KuiSpec outer = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}};
    kui_open(ui, &outer, NULL);
    KuiSpec page = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
                    .overflow = KUI_SCROLL_Y, .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR("page"), &page, NULL);
    for (int i = 0; i < CONF_LAYERS_ROWS; i++) {
        char name[16];
        snprintf(name, sizeof name, "row%d", i);
        KuiSpec row = {.dir = KUI_ROW, .width = {KUI_GROW, 1}, .height = {KUI_FIXED, 30},
                       .bg = i % 2 == 0 ? 0x22242cff : 0x30344aff};
        kui_open_keyed(ui, KUI_STR(name), &row, NULL);
        kui_close(ui);
    }
    kui_close(ui);
    if (phase != 1) {
        conf_layers_float(ui, "popover", 200, 40, 0x3b5bd4ff, "popover", "Popover");
    }
    conf_layers_float(ui, "toast", 140, 60, 0x73d98cff, "toast", "Toast");
    kui_close(ui);
}

/* conformance::build_drop (ADR 0031): two zones, a button inside the
 * first, and across the phases a hoverable float over the first zone that
 * is no zone (phase 1) and a modal over it (phase 2). The tags are
 * borrowed for the open call, like `modal`. */
static void conf_drop(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    KuiSpec outer = {.dir = KUI_ROW, .width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}};
    kui_open(ui, &outer, NULL);
    KuiValue *files = kui_value_map();
    kui_value_map_set(files, KUI_STR("kind"), kui_value_str(KUI_STR("files")));
    KuiSpec zone = {
        .width = {KUI_FIXED, 200}, .height = {KUI_GROW, 1},
        .pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10,
        .bg = 0x22242cff, .drop_bg = 0x2b3350ff, .on_drop = files,
    };
    kui_open_keyed(ui, KUI_STR("files"), &zone, NULL);
    {
        KuiValue *pick = kui_value_map();
        kui_value_map_set(pick, KUI_STR("kind"), kui_value_str(KUI_STR("pick")));
        KuiSpec button = {.dir = KUI_ROW, .width = {KUI_FIXED, 60}, .height = {KUI_FIXED, 40},
                          .bg = 0x3b5bd4ff, .label = KUI_STR("Pick")};
        kui_open_keyed(ui, KUI_STR("pick"), &button, pick);
        kui_close(ui);
    }
    kui_close(ui);
    kui_value_free(files);
    KuiValue *other = kui_value_map();
    kui_value_map_set(other, KUI_STR("kind"), kui_value_str(KUI_STR("other")));
    KuiSpec second = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
                      .bg = 0x30344aff, .on_drop = other};
    kui_open_keyed(ui, KUI_STR("other"), &second, NULL);
    kui_close(ui);
    kui_value_free(other);
    if (phase == 1) {
        KuiSpec overlay = {
            .float_mode = KUI_FLOAT_VIEWPORT,
            .float_anchor_x = KUI_START, .float_anchor_y = KUI_START,
            .float_self_x = KUI_START, .float_self_y = KUI_START,
            .float_dx = 20, .float_dy = 20,
            .width = {KUI_FIXED, 160}, .height = {KUI_FIXED, 160}, .hoverable = 1,
        };
        kui_open_keyed(ui, KUI_STR("overlay"), &overlay, NULL);
        kui_close(ui);
    }
    if (phase == 2) {
        KuiValue *modal = kui_value_map();
        kui_value_map_set(modal, KUI_STR("kind"), kui_value_str(KUI_STR("dismiss")));
        KuiSpec confirm = {
            .float_mode = KUI_FLOAT_VIEWPORT,
            .float_anchor_x = KUI_START, .float_anchor_y = KUI_START,
            .float_self_x = KUI_START, .float_self_y = KUI_START,
            .float_dx = 20, .float_dy = 20,
            .width = {KUI_FIXED, 160}, .height = {KUI_FIXED, 160},
            .bg = 0x101018ff, .modal = modal,
        };
        kui_open_keyed(ui, KUI_STR("confirm"), &confirm, NULL);
        kui_close(ui);
        kui_value_free(modal);
    }
    kui_close(ui);
}

/* conformance::build_sampler (backlog AR47): the generic rows no other
 * scene declares, on four nodes. `center` is both alignments at
 * KUI_CENTER here, as the Rust builder's center() is; the per-corner
 * radii need per_corner set; the caret and the anchor ride value_set. */
static void conf_sampler(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)phase;
    KuiSpec outer = {.pad_l = 8, .pad_r = 8, .pad_t = 8, .pad_b = 8, .gap = 6};
    kui_open(ui, &outer, NULL);
    KuiValue *click = kui_value_map();
    kui_value_map_set(click, KUI_STR("kind"), kui_value_str(KUI_STR("card")));
    KuiValue *hover = kui_value_map();
    kui_value_map_set(hover, KUI_STR("kind"), kui_value_str(KUI_STR("hov")));
    KuiValue *layout = kui_value_map();
    kui_value_map_set(layout, KUI_STR("kind"), kui_value_str(KUI_STR("lay")));
    KuiValue *force = kui_value_map();
    kui_value_map_set(force, KUI_STR("kind"), kui_value_str(KUI_STR("force")));
    KuiKeyframe stops[] = {
        {.set = KUI_KF_BG, .bg = 0x1b1d27ff},
        {.set = KUI_KF_AT | KUI_KF_BG | KUI_KF_RADIUS, .at = 1, .bg = 0x3b5bd4ff, .radius = 12},
    };
    KuiSpec card = {
        .dir = KUI_ROW,
        .width = {KUI_FIXED, 120}, .height = {KUI_FIXED, 40},
        .max_w = 100, .max_h = 30,
        .main_align = KUI_CENTER, .cross_align = KUI_CENTER,
        .bg = 0x1b1d27ff,
        .per_corner = 1, .radius_tl = 8, .radius_tr = 2, .radius_br = 8, .radius_bl = 2,
        .shadow_color = 0x00000080, .shadow_x = 3, .shadow_y = 2, .shadow_blur = 2,
        .hoverable = 1, .hover_bg = 0x262a3aff, .pressed_bg = 0x30364aff,
        .hover_group = KUI_STR("cards"),
        .focusable = 1, .focus_bg = 0x2b3350ff, .initial_focus = 1,
        .accent = 1,
        .cursor = KUI_CURSOR_POINTER,
        .selected = 1, .expanded = KUI_EXPANDED_EXPANDED,
        .on_layout = layout, .on_force_click = force,
        .click_sound = f->sound, .hover_sound = f->sound,
        .animate = 1,
        .transition_ms = 100, .easing = KUI_EASE_IN_OUT, .slide = 1,
        .delay_ms = 20, .repeat = KUI_REPEAT_ALTERNATE,
        .keyframes = stops, .keyframes_len = 2,
        .enter = {.set = KUI_ENTER_OFFSET | KUI_ENTER_OPACITY, .dx = -12, .opacity = 0},
        .role = KUI_ROLE_TAB, .label = KUI_STR("Card"),
    };
    kui_open_with(ui, KUI_STR("card"), &card, click, NULL, NULL, hover);
    KuiTextStyle s12 = {.size = 12};
    kui_text(ui, KUI_STR("ab"), &s12);
    kui_close(ui);
    kui_value_free(layout);
    kui_value_free(force);
    KuiSpec strip = {.dir = KUI_ROW, .width = {KUI_FIXED, 60}, .height = {KUI_FIXED, 10},
                     .bg = 0x3a3f52ff, .window_role = KUI_WINDOW_DRAG};
    kui_open_keyed(ui, KUI_STR("strip"), &strip, NULL);
    kui_close(ui);
    KuiSpec dock = {.dir = KUI_ROW, .focus_region = 1, .gap = 4, .height = {KUI_FIXED, 30},
                    .main_align = KUI_CENTER, .cross_align = KUI_END};
    kui_open_keyed(ui, KUI_STR("dock"), &dock, NULL);
    KuiSpec stop = {.dir = KUI_ROW, .width = {KUI_FIXED, 20}, .height = {KUI_FIXED, 20},
                    .bg = 0x2a2d3aff, .focusable = 1, .role = KUI_ROLE_BUTTON,
                    .label = KUI_STR("Stop")};
    kui_open_keyed(ui, KUI_STR("stop"), &stop, NULL);
    kui_close(ui);
    kui_close(ui);
    KuiSpec narrow = {.width = {KUI_FIXED, 60}};
    kui_open(ui, &narrow, NULL);
    KuiTextStyle cut = {.size = 12, .max_lines = 1, .ellipsis = 1,
                        .features = KUI_STR("liga=0"),
                        .decoration = KUI_DECO_UNDERLINE | KUI_DECO_STRIKETHROUGH};
    kui_text(ui, KUI_STR("a long line that is cut short"), &cut);
    kui_close(ui);
    KuiSpec line = {.dir = KUI_ROW, .height = {KUI_FIXED, 16}, .role = KUI_ROLE_LINE,
                    .value_set = KUI_VALUE_CARET | KUI_VALUE_ANCHOR | KUI_VALUE_CARET_SOLID,
                    .caret = 2, .selection_anchor = 0};
    kui_open_keyed(ui, KUI_STR("line"), &line, NULL);
    kui_text(ui, KUI_STR("sel"), &s12);
    kui_close(ui);
    kui_close(ui);
}

/* conformance::build_select: the stock select over four rows, the fourth
 * posting an id and disabled, the second in force. The rows are the
 * KuiMenuItems a context menu takes; `id` is borrowed for the call. */
static void conf_select(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiValue *la = kui_value_str(KUI_STR("la"));
    KuiMenuItem langs[] = {
        {.label = KUI_STR("English"), .role = KUI_MENU_CUSTOM, .enabled = 1},
        {.label = KUI_STR("Deutsch"), .role = KUI_MENU_CUSTOM, .enabled = 1},
        {.label = KUI_STR("Fran\xc3\xa7" "ais"), .role = KUI_MENU_CUSTOM, .enabled = 1},
        {.label = KUI_STR("Latin"), .role = KUI_MENU_CUSTOM, .enabled = 0, .id = la},
    };
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10, .gap = 6};
    kui_open(ui, &outer, NULL);
    kui_select(ui, KUI_STR("language"), langs, 4, 1);
    kui_value_free(la);
    KuiTextStyle body = {.size = 12};
    kui_text(ui, KUI_STR("body"), &body);
    kui_close(ui);
}

/* conformance::build_menu_bar: the application menu bar (ADR 0018) - a
 * declaration, and the widget that draws it. The rows are the KuiMenuItems
 * a context menu takes, one level down; `id` values are borrowed for the
 * length of the declaring call, so they are freed right after it. */
static void conf_menu_bar(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiValue *new_id = kui_value_str(KUI_STR("file.new"));
    KuiValue *wrap_id = kui_value_str(KUI_STR("file.wrap"));
    KuiValue *print_id = kui_value_str(KUI_STR("file.print"));
    KuiMenuItem file[] = {
        {.label = KUI_STR("New"), .role = KUI_MENU_CUSTOM, .enabled = 1,
         .id = new_id, .accel = KUI_STR("mod+n")},
        {.role = KUI_MENU_SEPARATOR},
        {.label = KUI_STR("Wrap"), .role = KUI_MENU_CUSTOM, .enabled = 1,
         .id = wrap_id, .checked = 1},
        {.label = KUI_STR("Print"), .role = KUI_MENU_CUSTOM, .enabled = 0,
         .id = print_id},
    };
    KuiMenuItem edit[] = {
        {.role = KUI_MENU_COPY, .enabled = 1},
    };
    KuiMenu menus[] = {
        {.label = KUI_STR("File"), .items = file, .count = 4, .enabled = 1},
        {.label = KUI_STR("Edit"), .items = edit, .count = 1, .enabled = 1},
    };
    KuiSpec outer = {.gap = 6, .width = {KUI_GROW, 1}};
    kui_open(ui, &outer, NULL);
    kui_menu_bar(ui, menus, 2);
    kui_value_free(new_id);
    kui_value_free(wrap_id);
    kui_value_free(print_id);
    KuiTextStyle body = {.size = 12};
    kui_text(ui, KUI_STR("body"), &body);
    kui_close(ui);
}

/* One entry per scene of conformance::SCENES; a scene in the reference with
 * no entry here fails the run rather than being skipped. */
static const ConfScene CONF_SCENES[] = {
    {"layout", conf_layout},
    {"sizing", conf_sizing},
    {"wrap", conf_wrap},
    {"align", conf_align},
    {"stock-controls", conf_stock_controls},
    {"table", conf_table},
    {"tabs", conf_tabs},
    {"overflow", conf_overflow},
    {"float", conf_float},
    {"clip-float", conf_clip_float},
    {"pixel-snap", conf_pixel_snap},
    {"clip-access", conf_clip_access},
    {"tooltip", conf_tooltip},
    {"select", conf_select},
    {"chrome", conf_chrome},
    /* Same builder: chrome-inset is the same tree under an env that also
     * reports the OS controls, so the two scenes differ only in the env. */
    {"chrome-inset", conf_chrome},
    {"controls", conf_controls},
    {"keys", conf_keys},
    {"ime", conf_ime},
    /* The paste scene's tree is the ime scene's: two editors, a paste's
     * markers heard by one and ignored by the other (backlog F84). */
    {"paste", conf_ime},
    {"cells", conf_cells},
    {"underlines", conf_underlines},
    {"joined-backgrounds", conf_joined_backgrounds},
    {"media", conf_media},
    {"lines", conf_lines},
    {"polygon", conf_polygon},
    {"fragments", conf_fragments},
    {"modal", conf_modal},
    {"composite", conf_composite},
    {"exit", conf_exit},
    {"windows", conf_windows},
    {"popup", conf_popup},
    {"live", conf_live},
    {"drag", conf_drag},
    {"selection", conf_selection},
    {"selection-extend", conf_selection},
    {"selection-scroll", conf_selection_scroll},
    {"cells-scroll", conf_cells_scroll},
    /* Same builder: `menu` is that tree under a secondary press, and what
     * it draws is the core's own menu rather than anything declared. */
    {"menu", conf_selection},
    {"menubar", conf_menu_bar},
    {"virtual", conf_virtual},
    {"layers", conf_layers},
    {"drop", conf_drop},
    {"anchor", conf_anchor},
    {"scrollbar", conf_scrollbar},
    {"tokens", conf_tokens},
    {"sampler", conf_sampler},
};

/* -- driving one scene --------------------------------------------------- */

/* A replayed input, parsed back out of the reference report's step lines so
 * the scenes need not restate it. The cap is a limit on one scene's replay,
 * not on the corpus. */
#define CONF_MAX_STEPS 32
typedef struct ConfStep {
    char kind[16];
    int a, b, c;
    /* How many numbers followed the kind: "phase" and "time" carry one,
     * "cursor" and "scroll" two, "dragfiles" and "dropfiles" three, the
     * rest none. */
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

/* Drains the audio commands into `audio`, one line each, the way
 * conformance::write_audio_command spells them: the verb and the playback,
 * plus a play's looped bit. Volumes, fades and the sound handle are left
 * out because they would not compare across four bindings. The queue is
 * drained a batch at a time, which is what a host driving its own device
 * does; 32 is far more than any corpus scene queues in one frame. */
static void conf_drain_audio(KuiCtx *ctx, Rep *audio) {
    KuiAudioCommand cmds[32];
    size_t n;
    while ((n = kui_take_audio_commands(ctx, cmds, 32)) > 0) {
        for (size_t i = 0; i < n; i++) {
            KuiAudioCommand c = cmds[i];
            switch (c.kind) {
            case KUI_AUDIO_PLAY:
                repf(audio, "audio play %llu %u\n", (unsigned long long)c.playback,
                     c.looped ? 1u : 0u);
                break;
            case KUI_AUDIO_STOP:
                repf(audio, "audio stop %llu\n", (unsigned long long)c.playback);
                break;
            case KUI_AUDIO_SET_VOLUME:
                repf(audio, "audio volume %llu\n", (unsigned long long)c.playback);
                break;
            case KUI_AUDIO_PAUSE:
                repf(audio, "audio pause %llu\n", (unsigned long long)c.playback);
                break;
            case KUI_AUDIO_RESUME:
                repf(audio, "audio resume %llu\n", (unsigned long long)c.playback);
                break;
            case KUI_AUDIO_MASTER_VOLUME: repf(audio, "audio master\n"); break;
            case KUI_AUDIO_UNLOAD: repf(audio, "audio unload\n"); break;
            default: repf(audio, "audio ? %llu\n", (unsigned long long)c.playback); break;
            }
        }
        if (n < 32) break;
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
    /* The modifier state as KUI_KMOD_* bits, the corpus's own spelling:
       what a Shift-press reads (ADR 0029). */
    else if (strcmp(s->kind, "modifiers") == 0) kui_input_modifiers(ctx, (uint32_t)s->a);
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
    /* An IME composing one character (any scalar value, so encoded) with
     * its caret at the end, or 0 for the composition ending; and committing
     * one. */
    else if (strcmp(s->kind, "preedit") == 0) {
        uint8_t b[4];
        size_t n = s->a ? conf_utf8((uint32_t)s->a, b) : 0;
        kui_input_preedit(ctx, (KuiStr){b, n}, n ? 0 : UINT32_MAX, n ? (uint32_t)n : UINT32_MAX);
    }
    else if (strcmp(s->kind, "commit") == 0) {
        uint8_t b[4];
        size_t n = conf_utf8((uint32_t)s->a, b);
        kui_input_commit(ctx, (KuiStr){b, n});
    }
    /* The clipboard answering a paste with one character and the
     * pasteboard's markers as KUI_PASTE_* bits (backlog F84). */
    else if (strcmp(s->kind, "paste") == 0) {
        uint8_t b[4];
        size_t n = conf_utf8((uint32_t)s->a, b);
        kui_input_paste(ctx, (KuiStr){b, n}, (uint32_t)s->b);
    }
    /* Files dragged in from the OS (ADR 0031): `a` files spelled
     * /drop/1.txt ... /drop/a.txt (conformance::drop_paths), at (b, c). */
    else if (strcmp(s->kind, "dragfiles") == 0 || strcmp(s->kind, "dropfiles") == 0) {
        char names[8][16];
        KuiStr paths[8];
        size_t n = s->a > 8 ? 8 : (size_t)s->a;
        for (size_t i = 0; i < n; i++) {
            snprintf(names[i], sizeof names[i], "/drop/%zu.txt", i + 1);
            paths[i] = (KuiStr){(const uint8_t *)names[i], strlen(names[i])};
        }
        if (s->kind[2] == 'a') kui_input_drag_files(ctx, paths, n, (float)s->b, (float)s->c);
        else kui_input_drop_files(ctx, paths, n, (float)s->b, (float)s->c);
    }
    else if (strcmp(s->kind, "dragcancel") == 0) kui_input_drag_cancel(ctx);
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
        /* A slider's change carries its phase and the value the core
         * worked out (docs/adr/0034); the corpus steps land on whole
         * values, which kui_value_as_int truncates to as the reference's
         * cast does. */
        if (kind.len == 6 && memcmp(kind.ptr, "change", 6) == 0) {
            KuiStr phase = KUI_STR("-");
            const KuiValue *p = kui_value_get(ev.payload, KUI_STR("phase"));
            if (p) kui_value_as_str(p, &phase);
            int64_t v = 0;
            const KuiValue *vv = kui_value_get(ev.payload, KUI_STR("value"));
            if (vv) kui_value_as_int(vv, &v);
            repf(events, " %.*s %lld", (int)phase.len, phase.ptr, (long long)v);
        }
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
        /* A scroll's lines ride the same way - the whole lines a grid's
         * notch covers, `-` off a grid - so a lost carry disagrees here
         * (ADR 0029). */
        if (kind.len == 6 && memcmp(kind.ptr, "scroll", 6) == 0) {
            const KuiValue *l = kui_value_get(ev.payload, KUI_STR("lines"));
            int64_t n = 0;
            if (l && kui_value_as_int(l, &n)) repf(events, " %lld", (long long)n);
            else repf(events, " -");
        }
        /* A drop's phase and its path count ride the same way (ADR 0031). */
        if (kind.len == 4 && memcmp(kind.ptr, "drop", 4) == 0) {
            KuiStr phase = KUI_STR("-");
            const KuiValue *p = kui_value_get(ev.payload, KUI_STR("phase"));
            if (p) kui_value_as_str(p, &phase);
            const KuiValue *l = kui_value_get(ev.payload, KUI_STR("paths"));
            repf(events, " %.*s %zu", (int)phase.len, phase.ptr, l ? kui_value_len(l) : (size_t)0);
        }
        /* A paste's markers ride the same way, each only when it is set
         * (backlog F84). */
        if (kind.len == 4 && memcmp(kind.ptr, "text", 4) == 0) {
            static const char *const markers[] = {"concealed", "transient"};
            for (int m = 0; m < 2; m++) {
                const KuiValue *v = kui_value_get(ev.payload, (KuiStr){(const uint8_t *)markers[m], strlen(markers[m])});
                bool on = false;
                if (v && kui_value_as_bool(v, &on) && on) repf(events, " %s", markers[m]);
            }
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

    Rep events, cmds, audio;
    rep_init(&events);
    rep_init(&cmds);
    rep_init(&audio);
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
                conf_drain_audio(ctx, &audio);
            } else if (strcmp(s->kind, "windowdismissed") == 0) {
                kui_window_dismissed(ctx, (uint32_t)s->a, (uint32_t)s->b);
                conf_drain(ctx, &events);
            } else if (strcmp(s->kind, "appearance") == 0) {
                /* The OS appearance moving under the app: the index is
                 * Appearance::ALL's, which is the code the C side takes;
                 * accent, motion and locale stay at "cannot tell". */
                kui_env_set_system(ctx, (uint32_t)s->a, 0, 0, (KuiStr){NULL, 0});
                conf_drain(ctx, &events);
            } else {
                conf_apply(ctx, s);
                conf_drain(ctx, &events);
                conf_drain_cmds(ctx, &cmds);
                conf_drain_audio(ctx, &audio);
            }
        }
        kui_frame_begin(ctx, 320, 240, 1);
        scene->build(ctx, &f, phase);
        kui_frame_finish(ctx);
        conf_drain(ctx, &events);
        conf_drain_cmds(ctx, &cmds);
        conf_drain_audio(ctx, &audio);
    }

    repf(out, "scene %s\n", scene->name);
    if (env->set) {
        repf(out, "env %d %d %d %d %d\n", env->custom_chrome, env->maximized,
             env->fullscreen, env->controls_w, env->controls_h);
    }
    for (int i = 0; i < nsteps; i++) {
        if (steps[i].args >= 3)
            repf(out, "step %s %d %d %d\n", steps[i].kind, steps[i].a, steps[i].b, steps[i].c);
        else if (steps[i].args == 2) repf(out, "step %s %d %d\n", steps[i].kind, steps[i].a, steps[i].b);
        else if (steps[i].args == 1) repf(out, "step %s %d\n", steps[i].kind, steps[i].a);
        else repf(out, "step %s\n", steps[i].kind);
    }

    KuiStr title;
    if (kui_window_title_get(ctx, &title)) repf(out, "title %.*s\n", (int)title.len, title.ptr);
    else repf(out, "title -\n");
    repf(out, "always-on-top %d\n", kui_always_on_top_get(ctx) ? 1 : 0);
    repf(out, "secure-input %d\n", kui_secure_input_get(ctx) ? 1 : 0);

    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ctx, &dd);
    repf(out, "quads %zu %016llx\n", dd.quad_count,
         (unsigned long long)quad_digest(dd.quads, dd.quad_count, dd.clips,
                                         dd.clip_count));
    size_t kinds[9] = {0};
    for (size_t i = 0; i < dd.quad_count; i++) {
        if (dd.quads[i].kind < 9) kinds[dd.quads[i].kind]++;
    }
    repf(out, "kinds %zu %zu %zu %zu %zu %zu %zu %zu %zu\n", kinds[0], kinds[1], kinds[2],
         kinds[3], kinds[4], kinds[5], kinds[6], kinds[7], kinds[8]);
    /* A fragment's parameters ride a side list, not the quad, so the digest
     * cannot reach them; the report carries them as bits, like the core's. */
    for (size_t i = 0; i < dd.fragment_count; i++) {
        repf(out, "fragment %zu", i);
        for (int j = 0; j < 16; j++) {
            uint32_t bits;
            memcpy(&bits, &dd.fragments[i].params[j], sizeof bits);
            repf(out, " %08x", bits);
        }
        repf(out, "\n");
    }
    /* Where a fragment's image is, for the draws that have one. */
    for (size_t i = 0; i < dd.fragment_count; i++) {
        const KuiFragmentDraw *d = &dd.fragments[i];
        if (d->image_source == KUI_FRAGMENT_IMAGE_NONE) continue;
        repf(out, "fragment-image %zu %s ", i,
             d->image_source == KUI_FRAGMENT_IMAGE_ATLAS ? "atlas" : "texture");
        if (d->image_source == KUI_FRAGMENT_IMAGE_TEXTURE) repf(out, "%u", d->image_texture);
        else repf(out, "-");
        repf(out, " %u %u %u %u\n", d->image_uv[0], d->image_uv[1], d->image_uv[2], d->image_uv[3]);
    }
    /* A texture quad's texel rect rides the side list the same way. */
    for (size_t i = 0; i < dd.texture_count; i++) {
        repf(out, "texture %zu %u %u %u %u\n", i, dd.textures[i].uv[0], dd.textures[i].uv[1],
             dd.textures[i].uv[2], dd.textures[i].uv[3]);
    }

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
        const char *checked = (a->flags & KUI_ACCESS_MIXED)       ? "m"
                              : !(a->flags & KUI_ACCESS_CHECKED_SET) ? "-"
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
        /* The rect as f32 bits, cut to the node's clip (backlog F93). */
        uint32_t rect[4];
        const float xywh[4] = {a->x, a->y, a->w, a->h};
        memcpy(rect, xywh, sizeof rect);
        repf(out, "node %d %016llx %s %d %d %s %s %s %s %d %08x %08x %08x %08x %s %.*s | %.*s | %.*s\n",
             depth, (unsigned long long)a->key, role_name(a->role),
             (a->flags & KUI_ACCESS_FOCUSED) ? 1 : 0,
             (a->flags & KUI_ACCESS_DISABLED) ? 1 : 0,
             checked, selected, orientation, live,
             (a->flags & KUI_ACCESS_HAS_SCROLL) ? 1 : 0,
             rect[0], rect[1], rect[2], rect[3],
             off ? actions : "-",
             (int)a->name.len, a->name.ptr,
             (int)a->description.len, a->description.ptr,
             (int)a->value.len, a->value.ptr);
    }

    repf(out, "%s", events.buf);
    rep_free(&events);
    repf(out, "%s", cmds.buf);
    rep_free(&cmds);
    repf(out, "%s", audio.buf);
    rep_free(&audio);

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
                s->a = s->b = s->c = 0;
                s->args = sscanf(line, "step %15s %d %d %d", s->kind, &s->a, &s->b, &s->c) - 1;
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

int main(int argc, char **argv) {
    if (!abi_ok()) return 1;
    return conformance(argc > 1 ? argv[1] : "target/conformance.txt");
}
