/* kui C API — see crates/kui-ffi/src/lib.rs for the implementation.
 *
 * Conventions:
 *  - Strings are UTF-8 (ptr, len) pairs; use KUI_STR("literal").
 *  - KuiValue* from kui_value_* constructors is owned by you until passed to
 *    a function documented as consuming it (on_click args, map_set values).
 *  - Event payloads from kui_poll_event are borrowed: valid until the next
 *    poll on the same context. Payloads inside callbacks are borrowed for the
 *    duration of the callback.
 *  - Coordinates are logical pixels; draw data comes back in physical pixels.
 */
#ifndef KUI_H
#define KUI_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KuiCtx KuiCtx;
typedef struct KuiValue KuiValue;

typedef struct KuiStr {
    const uint8_t *ptr;
    size_t len;
} KuiStr;

#define KUI_STR(s) ((KuiStr){(const uint8_t *)(s), strlen(s)})

/* Sizing tags */
enum { KUI_FIT = 0, KUI_GROW = 1, KUI_FIXED = 2, KUI_PERCENT = 3 };
/* Directions */
enum { KUI_COLUMN = 0, KUI_ROW = 1 };
/* Alignment */
enum { KUI_START = 0, KUI_CENTER = 1, KUI_END = 2 };
/* Quad kinds */
enum { KUI_QUAD_SOLID = 0, KUI_QUAD_GLYPH_MASK = 1, KUI_QUAD_GLYPH_COLOR = 2 };
/* Span flags */
enum { KUI_SPAN_BOLD = 1u << 0, KUI_SPAN_ITALIC = 1u << 1 };
/* Overflow flags */
enum { KUI_CLIP = 1u << 0, KUI_SCROLL_X = 1u << 1, KUI_SCROLL_Y = 1u << 2 };

typedef struct KuiSizing {
    uint32_t tag;
    float value;
} KuiSizing;

/* Zero-initialized KuiSpec is a fit-sized transparent column. Colors are
 * 0xRRGGBBAA with 0 meaning "none". */
typedef struct KuiSpec {
    KuiSizing width, height;
    float min_w, max_w, min_h, max_h; /* clamps; max 0 = unconstrained */
    uint32_t dir;
    float pad_l, pad_r, pad_t, pad_b;
    float gap;
    uint32_t main_align, cross_align;
    uint32_t bg;
    uint32_t border_color;
    float border_w;
    float radius;
    uint32_t overflow; /* KUI_CLIP | KUI_SCROLL_X | KUI_SCROLL_Y */
} KuiSpec;

/* Zero-initialized KuiTextStyle picks defaults (16px, default foreground). */
typedef struct KuiTextStyle {
    float size;
    float line_height; /* <= 0: default (size * 1.35) */
    uint32_t color;    /* 0: default foreground */
} KuiTextStyle;

typedef struct KuiSpan {
    KuiStr text;
    uint32_t color; /* 0: inherit paragraph color */
    uint32_t flags; /* KUI_SPAN_* */
} KuiSpan;

typedef struct KuiEvent {
    uint16_t origin;
    uint64_t key;
    const KuiValue *payload; /* borrowed; may be NULL */
} KuiEvent;

typedef struct KuiQuad {
    float x, y, w, h;        /* physical pixels */
    float color[4];
    float border_color[4];
    float radius;
    float border_w;
    uint32_t kind;           /* KUI_QUAD_* */
    uint32_t uv[4];          /* atlas texels: x, y, w, h */
    float clip[4];           /* clip rect (physical px): pixels outside are transparent */
} KuiQuad;

typedef struct KuiDrawData {
    const KuiQuad *quads;
    size_t quad_count;
    float viewport_w, viewport_h; /* physical pixels */
    float scale;
    const uint8_t *atlas_pixels;  /* RGBA, atlas_size^2 * 4 bytes */
    uint32_t atlas_size;
    bool atlas_dirty;             /* re-upload when set or epoch changed */
    uint64_t atlas_epoch;
} KuiDrawData;

/* -- Context ------------------------------------------------------------- */
KuiCtx *kui_ctx_new(void);
void kui_ctx_free(KuiCtx *ctx);

/* -- Input (logical coordinates) + events -------------------------------- */
void kui_input_cursor(KuiCtx *ctx, float x, float y);
void kui_input_cursor_left(KuiCtx *ctx);
void kui_input_mouse(KuiCtx *ctx, bool down);
void kui_input_scroll(KuiCtx *ctx, float dx, float dy); /* +y = scroll up */
bool kui_poll_event(KuiCtx *ctx, KuiEvent *out);

/* -- Frame building ------------------------------------------------------ */
void kui_frame_begin(KuiCtx *ctx, float w, float h, float scale);
void kui_root(KuiCtx *ctx, const KuiSpec *spec);
/* on_click may be NULL; consumed when given. Returns the node key. */
uint64_t kui_open(KuiCtx *ctx, const KuiSpec *spec, KuiValue *on_click);
uint64_t kui_open_keyed(KuiCtx *ctx, KuiStr label, const KuiSpec *spec, KuiValue *on_click);
void kui_close(KuiCtx *ctx);
void kui_text(KuiCtx *ctx, KuiStr text, const KuiTextStyle *style);
void kui_rich_text(KuiCtx *ctx, const KuiSpan *spans, size_t span_count,
                   const KuiTextStyle *base);
uint64_t kui_child_key(KuiCtx *ctx, KuiStr label);
bool kui_is_hovered(KuiCtx *ctx, uint64_t key);
bool kui_is_pressed(KuiCtx *ctx, uint64_t key);
/* Styled button with hover/press states; payload consumed (may be NULL). */
void kui_button(KuiCtx *ctx, KuiStr label, KuiValue *payload);
void kui_frame_finish(KuiCtx *ctx);
/* Pointers valid until the next kui_frame_begin on this context. */
void kui_draw_data(KuiCtx *ctx, KuiDrawData *out);

/* -- Values -------------------------------------------------------------- */
KuiValue *kui_value_null(void);
KuiValue *kui_value_bool(bool v);
KuiValue *kui_value_int(int64_t v);
KuiValue *kui_value_float(double v);
KuiValue *kui_value_str(KuiStr s);
KuiValue *kui_value_map(void);
void kui_value_map_set(KuiValue *map, KuiStr key, KuiValue *val); /* consumes val */
const KuiValue *kui_value_get(const KuiValue *v, KuiStr key);     /* borrowed */
bool kui_value_as_int(const KuiValue *v, int64_t *out);
bool kui_value_as_str(const KuiValue *v, KuiStr *out);            /* borrowed */
void kui_value_free(KuiValue *v);

/* -- Windowed runner (winit + wgpu), blocks until the window closes ------ */
typedef void (*KuiViewFn)(void *user, KuiCtx *ctx);
typedef void (*KuiEventFn)(void *user, const KuiEvent *ev);
bool kui_run(KuiStr title, KuiViewFn view, KuiEventFn on_event, void *user);

#ifdef __cplusplus
}
#endif

#endif /* KUI_H */
