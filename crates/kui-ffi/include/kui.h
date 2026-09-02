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
/* KUI_QUAD_GLYPH_SUBPIXEL: atlas rgb are per-channel coverages (needs
 * per-channel / dual-source blending; else use the atlas alpha as a mask).
 * Only produced after kui_set_subpixel_text(ctx, true). */
enum { KUI_QUAD_SOLID = 0, KUI_QUAD_GLYPH_MASK = 1, KUI_QUAD_GLYPH_COLOR = 2,
       KUI_QUAD_IMAGE = 3, KUI_QUAD_GLYPH_SUBPIXEL = 4 };
/* Font families (KuiTextStyle.family) */
enum { KUI_FONT_SANS = 0, KUI_FONT_SERIF = 1, KUI_FONT_MONO = 2 };
/* Span flags */
enum { KUI_SPAN_BOLD = 1u << 0, KUI_SPAN_ITALIC = 1u << 1 };
/* Overflow flags */
enum { KUI_CLIP = 1u << 0, KUI_SCROLL_X = 1u << 1, KUI_SCROLL_Y = 1u << 2 };
/* Editing keys (kui_input_key) */
enum {
    KUI_KEY_LEFT = 0, KUI_KEY_RIGHT, KUI_KEY_UP, KUI_KEY_DOWN,
    KUI_KEY_HOME, KUI_KEY_END, KUI_KEY_PAGE_UP, KUI_KEY_PAGE_DOWN,
    KUI_KEY_BACKSPACE, KUI_KEY_DELETE, KUI_KEY_ENTER, KUI_KEY_TAB,
    KUI_KEY_SELECT_ALL, KUI_KEY_ESCAPE, KUI_KEY_UNDO, KUI_KEY_REDO,
};
/* Modifier bits (kui_input_key) */
enum { KUI_MOD_SHIFT = 1u << 0, KUI_MOD_WORD = 1u << 1, KUI_MOD_DOC = 1u << 2 };
/* Physical modifier bits (kui_input_modifiers) */
enum {
    KUI_KMOD_SHIFT = 1u << 0,
    KUI_KMOD_CTRL = 1u << 1,
    KUI_KMOD_ALT = 1u << 2,
    KUI_KMOD_SUPER = 1u << 3,
};
/* Text edit flags (kui_text_edit) */
enum { KUI_EDIT_MULTILINE = 1u << 0, KUI_EDIT_AUTOFOCUS = 1u << 1 };
/* Float modes (KuiSpec.float_mode) */
enum { KUI_FLOAT_NONE = 0, KUI_FLOAT_PARENT = 1, KUI_FLOAT_VIEWPORT = 2 };
/* Window-chrome roles (KuiSpec.window_role). Chrome nodes turn input into
 * window commands (kui_take_window_commands), never events. */
enum {
    KUI_WINDOW_NONE = 0,
    KUI_WINDOW_DRAG = 1,
    KUI_WINDOW_CLOSE = 2,
    KUI_WINDOW_MINIMIZE = 3,
    KUI_WINDOW_MAXIMIZE = 4,
};
/* Easing curves (KuiSpec.easing). */
enum {
    KUI_EASE_OUT = 0,
    KUI_EASE_LINEAR = 1,
    KUI_EASE_IN = 2,
    KUI_EASE_IN_OUT = 3,
    KUI_EASE_SPRING = 4, /* damped spring; transition_ms is the response time */
    KUI_EASE_BOUNCY = 5,
};
/* Window commands drained by kui_take_window_commands. */
enum {
    KUI_CMD_START_DRAG = 1,
    KUI_CMD_CLOSE = 2,
    KUI_CMD_MINIMIZE = 3,
    KUI_CMD_TOGGLE_MAXIMIZE = 4,
};

typedef struct KuiSizing {
    uint32_t tag;
    float value;
} KuiSizing;

/* How keyframes cycle (KuiSpec.repeat): CSS animation-direction. */
enum {
    KUI_REPEAT_NORMAL = 0,
    KUI_REPEAT_REVERSE = 1,
    KUI_REPEAT_ALTERNATE = 2,
    KUI_REPEAT_ALTERNATE_REVERSE = 3,
};
/* Which KuiKeyframe fields are set (KuiKeyframe.set bits). */
enum {
    KUI_KF_AT = 1u << 0,
    KUI_KF_WIDTH = 1u << 1,
    KUI_KF_HEIGHT = 1u << 2,
    KUI_KF_BG = 1u << 3,
    KUI_KF_RADIUS = 1u << 4,
};

/* One CSS-style keyframe stop. A zeroed stop sets nothing: `set` says which
 * fields count, so 0 stays a legal value for each. Stops without KUI_KF_AT
 * spread evenly (a lone stop sits at 1 and animates from the node's own
 * value); declared `at`s must not decrease. Sizings animate their amount
 * only, in the form the spec's own width/height declares. */
typedef struct KuiKeyframe {
    uint32_t set;
    float at; /* 0..1 */
    KuiSizing width, height;
    uint32_t bg; /* 0xRRGGBBAA */
    float radius;
} KuiKeyframe;

/* Zero-initialized KuiSpec is a fit-sized transparent column. Colors are
 * 0xRRGGBBAA with 0 meaning "none". Fields mirror the shared prop schema
 * (crates/kui-core/src/schema.rs) and are append-only: the layout is ABI. */
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
    /* Out-of-flow positioning: 0 = in flow, KUI_FLOAT_PARENT/VIEWPORT anchors.
     * Attach points use KUI_START/CENTER/END; dx/dy is a logical-px offset. */
    uint32_t float_mode;
    uint32_t float_anchor_x, float_anchor_y;
    uint32_t float_self_x, float_self_y;
    float float_dx, float_dy;
    uint32_t float_fit; /* non-zero: flip across the anchor / clamp to stay in the viewport */
    uint32_t hoverable; /* non-zero: hover-track without a click payload (kui_is_hovered) */
    uint32_t window_role; /* KUI_WINDOW_*; makes this node window chrome */
    /* > 0: ease sizing/color/radius changes over this many ms (needs a stable
     * key via kui_open_keyed, and kui_set_time each frame). */
    float transition_ms;
    uint32_t easing; /* KUI_EASE_* */
    uint32_t slide;  /* non-zero: also ease the position (siblings slide) */
    /* Declarative pointer styling, resolved by the core when the node opens
     * (no kui_is_hovered round trip; eases with transition_ms). 0 = none.
     * Any of these makes the node hover-tracked. */
    uint32_t hover_bg;   /* 0xRRGGBBAA while hovered (or its hover group is) */
    uint32_t pressed_bg; /* 0xRRGGBBAA while pressed */
    /* Hover group name (empty = none): members show hover_bg / pressed_bg
     * together (a split button, a multi-piece shape). Hashed, not retained. */
    KuiStr hover_group;
    /* Per-corner radii: with per_corner non-zero, radius_tl..radius_bl are
     * the four corner radii (clockwise from the top-left) and `radius` is
     * ignored; zero keeps the uniform `radius` on every corner. */
    uint32_t per_corner;
    float radius_tl, radius_tr, radius_br, radius_bl;
    /* CSS-style keyframes: the slots the stops name (width, height, bg,
     * radius) cycle through them over transition_ms — forever, without the
     * view redrawing, sampled off kui_set_time's clock — in the `repeat`
     * direction (KUI_REPEAT_*), held back by delay_ms so siblings given
     * different delays run out of phase. Slots no stop names still ease
     * toward what the spec declares. The array is read while the node
     * opens and not retained; NULL / 0 = none. */
    uint32_t repeat;
    float delay_ms;
    const KuiKeyframe *keyframes;
    size_t keyframes_len;
} KuiSpec;

/* Zero-initialized KuiTextStyle picks defaults (16px, default foreground). */
typedef struct KuiTextStyle {
    float size;
    float line_height; /* <= 0: default (size * 1.35) */
    uint32_t color;    /* 0: default foreground */
    uint32_t family;   /* KUI_FONT_* ; 0 = sans */
    uint64_t font;     /* registered font handle (kui_font_add*); non-zero overrides family */
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
    float radius[4];         /* corner radii, clockwise from the top-left */
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
/* clicks: host-counted multi-click for presses (1 single, 2 double = word
 * select in editors, 3 triple = line select); ignored on release. */
void kui_input_mouse(KuiCtx *ctx, bool down, uint32_t clicks);
void kui_input_scroll(KuiCtx *ctx, float dx, float dy); /* +y = scroll up */
void kui_input_text(KuiCtx *ctx, KuiStr text);   /* typing/paste -> focused editor */
/* In-progress IME composition shown at the focused caret; empty text clears
 * it, the commit arrives via kui_input_text. cursor_* are byte offsets into
 * text (UINT32_MAX = none). */
void kui_input_preedit(KuiCtx *ctx, KuiStr text, uint32_t cursor_start,
                       uint32_t cursor_end);
void kui_input_key(KuiCtx *ctx, uint32_t key, uint32_t mods); /* KUI_KEY_* + KUI_MOD_* */
/* Physical modifier state changed (KUI_KMOD_* bits); the host polls a
 * {kind="modifiers", shift, ctrl, alt, super} event when it differs. */
void kui_input_modifiers(KuiCtx *ctx, uint32_t mods);
bool kui_poll_event(KuiCtx *ctx, KuiEvent *out);

/* -- Host environment ---------------------------------------------------- */
/* Host facts for views to read (refresh_hz <= 0 = unknown). Survives across
 * frames; set on change or every frame, either works. */
void kui_env_set(KuiCtx *ctx, float refresh_hz, bool focused);
/* The frame clock for transitions (monotonic seconds, any origin). Set before
 * each kui_frame_begin; never setting it makes transitions snap. */
void kui_set_time(KuiCtx *ctx, double now_secs);
/* True when the last frame left a transition mid-flight: draw another frame
 * without waiting for input. */
bool kui_animating(KuiCtx *ctx);
/* Rasterize outline glyphs as LCD subpixel coverage (KUI_QUAD_GLYPH_SUBPIXEL)
 * instead of alpha masks. Only turn it on if your renderer blends per
 * channel. Flipping it re-rasterizes every glyph. */
void kui_set_subpixel_text(KuiCtx *ctx, bool on);
/* Window chrome facts for views (widgets adapt to them). controls_w/h > 0
 * describe the top-left keep-out rect of OS-drawn controls (macOS traffic
 * lights under custom chrome). */
void kui_env_set_window(KuiCtx *ctx, bool custom_chrome, bool maximized,
                        bool fullscreen, float controls_w, float controls_h);
/* Drains pending window commands (KUI_CMD_*) into out, returns the count
 * written. Call after each input dispatch and apply to the real window. */
size_t kui_take_window_commands(KuiCtx *ctx, uint32_t *out, size_t cap);
/* Declares this frame's window title (cleared each kui_frame_begin). */
void kui_window_title(KuiCtx *ctx, KuiStr title);
/* The title declared this frame, if any — diff and apply after
 * kui_frame_finish. The view is valid until the next kui_frame_begin. */
bool kui_window_title_get(KuiCtx *ctx, KuiStr *out);

/* -- Frame building ------------------------------------------------------ */
/* w/h are logical pixels. A frame begun at a different size or scale than
 * the last one posts a {kind="resize", width, height, scale} event on the
 * root, polled after kui_frame_finish like any other. */
void kui_frame_begin(KuiCtx *ctx, float w, float h, float scale);
void kui_root(KuiCtx *ctx, const KuiSpec *spec);
/* on_click may be NULL; consumed when given. Returns the node key. */
uint64_t kui_open(KuiCtx *ctx, const KuiSpec *spec, KuiValue *on_click);
uint64_t kui_open_keyed(KuiCtx *ctx, KuiStr label, const KuiSpec *spec, KuiValue *on_click);
/* Draggable container: press-drag emits {kind="drag", phase="start"|"move"|
 * "end", x, y, dx, dy, tag} events; a drag past the click slop suppresses
 * on_click. on_click/on_drag are nullable and consumed. */
uint64_t kui_open_draggable(KuiCtx *ctx, KuiStr label, const KuiSpec *spec,
                            KuiValue *on_click, KuiValue *on_drag);
/* The general container: every message prop at once, each nullable and
 * consumed. NULL means absent (a NULL on_drag here does NOT make the node
 * draggable, unlike kui_open_draggable). A non-NULL on_key makes the node a
 * key sink: focus it with kui_set_key_focus and every press arrives as
 * {kind="key", code, ctrl, alt, shift, super, text, repeat, tag}. A non-NULL
 * on_hover makes the pointer entering/leaving emit
 * {kind="hover", phase="enter"|"leave", tag} — for hover-dependent layout;
 * plain hover colors belong in KuiSpec.hover_bg / pressed_bg. */
uint64_t kui_open_with(KuiCtx *ctx, KuiStr label, const KuiSpec *spec,
                       KuiValue *on_click, KuiValue *on_drag, KuiValue *on_key,
                       KuiValue *on_hover);
/* Routes the keyboard at a key-sink node for this frame (0 clears). Declare
 * it every frame you want it, like the title; a focused editor still wins. */
void kui_set_key_focus(KuiCtx *ctx, uint64_t key);
/* -- Fonts ---------------------------------------------------------------- */
/* Registers a font from file bytes (TTF/OTF/TTC, copied); returns a handle
 * for KuiTextStyle.font, 0 when the data holds no usable face. */
uint64_t kui_font_add(KuiCtx *ctx, const uint8_t *data, size_t len);
/* The handle for a font family by name ("Menlo") — installed, or loaded with
 * the two calls below; 0 when none matches. Idempotent per family. */
uint64_t kui_font_add_system(KuiCtx *ctx, KuiStr name);
/* Registers a font file by path (memory-mapped); 0 on failure. */
uint64_t kui_font_load_file(KuiCtx *ctx, KuiStr path);
/* Loads every font file under a folder (recursively) for kui_font_add_system;
 * returns the number of faces added. */
size_t kui_font_load_dir(KuiCtx *ctx, KuiStr dir);
/* Forgets a font; styles still naming it shape as sans. */
void kui_font_remove(KuiCtx *ctx, uint64_t id);
/* -- Images --------------------------------------------------------------- */
/* Registers a w*h RGBA image (pixels copied); returns a handle, 0 on
 * failure. Handles are stable until kui_image_remove. */
uint64_t kui_image_add(KuiCtx *ctx, uint32_t w, uint32_t h, const uint8_t *rgba);
void kui_image_remove(KuiCtx *ctx, uint64_t id);
/* An image node. Fit sizing = the image's pixel size as logical px; a Fit
 * height against a resolved width keeps the aspect; radius rounds corners. */
void kui_image(KuiCtx *ctx, uint64_t id, const KuiSpec *spec);
void kui_close(KuiCtx *ctx);
void kui_text(KuiCtx *ctx, KuiStr text, const KuiTextStyle *style);
void kui_rich_text(KuiCtx *ctx, const KuiSpan *spans, size_t span_count,
                   const KuiTextStyle *base);
uint64_t kui_child_key(KuiCtx *ctx, KuiStr label);
bool kui_is_hovered(KuiCtx *ctx, uint64_t key);
bool kui_is_pressed(KuiCtx *ctx, uint64_t key);
/* Styled button with hover/press states; payload consumed (may be NULL). */
void kui_button(KuiCtx *ctx, KuiStr label, KuiValue *payload);
/* -- Widgets (the same kui_core::widgets every frontend uses) ------------ */
/* Body callbacks build content through the same ctx (see KuiViewFn). */
typedef void (*KuiViewFn)(void *user, KuiCtx *ctx);
/* Adaptive titlebar: drag strip, title, window buttons per the env facts. */
void kui_titlebar(KuiCtx *ctx, KuiStr title);
/* Titlebar hosting custom content built by body (tabs, search, ...). */
void kui_titlebar_with(KuiCtx *ctx, KuiViewFn body, void *user);
/* Min/max/close cluster; draws nothing when the OS provides controls. */
void kui_window_buttons(KuiCtx *ctx);
/* Hint floated below the enclosing node; gate on kui_is_hovered. */
void kui_tooltip(KuiCtx *ctx, KuiStr text);
void kui_tooltip_with(KuiCtx *ctx, KuiViewFn body, void *user);
/* Per-phase frame-latency bars (populated by kui_run; empty headless). */
void kui_latency_graph(KuiCtx *ctx);
/* The graph in a corner panel; x/y are KUI_START/CENTER/END. */
void kui_latency_hud(KuiCtx *ctx, uint32_t x, uint32_t y);
/* Single-line input with chrome; returns the editor key (kui_edit_text). */
uint64_t kui_text_input(KuiCtx *ctx, KuiStr label, KuiStr initial);
/* Editable text node (state retained by key). Returns the node key;
 * "changed"/"submit" events arrive via kui_poll_event with that key. */
uint64_t kui_text_edit(KuiCtx *ctx, KuiStr label, KuiStr initial,
                       const KuiTextStyle *style, uint32_t flags, const KuiSpec *spec);
/* Borrowed view of an editor's text; valid until the next kui_edit_text call. */
bool kui_edit_text(KuiCtx *ctx, uint64_t key, KuiStr *out);
void kui_edit_set_text(KuiCtx *ctx, uint64_t key, KuiStr text);
bool kui_is_focused(KuiCtx *ctx, uint64_t key);
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
typedef void (*KuiEventFn)(void *user, const KuiEvent *ev);
bool kui_run(KuiStr title, KuiViewFn view, KuiEventFn on_event, void *user);

#ifdef __cplusplus
}
#endif

#endif /* KUI_H */
