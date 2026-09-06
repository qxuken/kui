/* kui C API — see crates/kui-ffi/src/ for the implementation (types.rs
 * mirrors these structs; each kui_* function's module is named for its
 * concern).
 *
 * Conventions:
 *  - Strings are UTF-8 (ptr, len) pairs; use KUI_STR("literal").
 *  - KuiValue* from kui_value_* constructors is owned by you until passed to
 *    a function documented as consuming it (on_click args, map_set values).
 *  - Event payloads from kui_poll_event are borrowed: valid until the next
 *    poll on the same context. Payloads inside callbacks are borrowed for the
 *    duration of the callback.
 *  - Coordinates are logical pixels; draw data comes back in physical pixels.
 *  - Every struct below is marked [in], [out], [out[]] or [lib], which says
 *    who allocates it and who writes it. Read "Who writes what" first.
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

/* -- ABI version ----------------------------------------------------------
 *
 * KUI_ABI_VERSION is the ABI this header describes; kui_abi_version() is the
 * one the library you actually loaded implements. They are settled against
 * each other at build time only if you built both, which a host that links
 * a shared libkui_ffi did not. Check them once, before anything else:
 *
 *     if (kui_abi_version() != KUI_ABI_VERSION) {
 *         fprintf(stderr, "libkui is ABI %u, this build wants %u\n",
 *                 kui_abi_version(), KUI_ABI_VERSION);
 *         return 1;
 *     }
 *
 * Equality, not >=: the mismatch this exists to catch is a *newer* library
 * against an older host, which is the direction that corrupts memory rather
 * than merely missing a feature.
 *
 * The number bumps when the layout of anything the library writes or
 * allocates changes - an [out], [out[]] or [lib] struct below, in any way,
 * appended fields included. It does not bump when an [in] struct gains a
 * field: the library only reads those, so a host that predates the field
 * passes a shorter struct and gets the zeroed default (KuiSpec grew
 * `tooltip` exactly this way). It does not bump for a new function either -
 * a host that does not call one is unaffected, and one that does fails to
 * link, which is loud.
 *
 * ABI 4 is the first release to append to an [out] struct: KuiEvent gained
 * `window`. If you set `size` (KUI_EVENT_INIT does) you need no source
 * change for it - the library writes the prefix your build reserved and
 * stops. The version still bumps, because a host that skipped this check
 * would otherwise get that short write without ever having asked for it.
 *
 * ABI 5 is multi-window (docs/adr/0004-multi-window.md, step 3), and its
 * two breaks are source breaks you see at compile time: the uint32_t array
 * kui_take_window_commands filled became the KuiWindowCommand [out] struct
 * kui_take_window_command pops (a command now names its window, and an
 * open carries a config), and kui_env_set_window leads with the window id.
 * Edit the drain loop and the env call; nothing else changes meaning.
 *
 * ABI 6 appends width/height to KuiWindowCommand, for the KUI_CMD_SET_SIZE
 * kui_set_window_size queues (ADR 0004 step 5). Set `size` (as
 * KUI_WINDOW_COMMAND_INIT does) and you need no source change: the library
 * writes the prefix your build reserved and stops. Nor can the new verb
 * reach a host that never calls kui_set_window_size - only that call
 * produces it. The version bumps for the host that skipped this check.
 */
#define KUI_ABI_VERSION 6u
uint32_t kui_abi_version(void);

/* -- Who writes what ------------------------------------------------------
 *
 * [in]     You allocate and fill it; the library reads it. Zero-initialize
 *          and set what you need - a zeroed field is the documented default.
 *          A later kui may append fields; your shorter struct is fine.
 *          KuiSpec, KuiSizing, KuiKeyframe, KuiEnter, KuiTextStyle, KuiSpan,
 *          KuiPlay, KuiAudio, KuiWindowConfig.
 *
 * [out]    You allocate it; the library WRITES it. These lead with a
 *          `uint32_t size` you set to sizeof the struct, and the library
 *          writes no further than that - which is what lets a later kui
 *          append a field without writing past the end of what your build
 *          reserved. Start from the KUI_*_INIT initializer:
 *
 *              KuiEvent ev = KUI_EVENT_INIT;
 *              while (kui_poll_event(ctx, &ev)) { ... }
 *
 *          A call whose `size` is below the ABI-1 layout (which is what a
 *          zeroed or never-set one looks like) writes nothing and returns
 *          false, rather than guessing; it also consumes nothing, so a
 *          refused kui_poll_event leaves the event queued (the payload from
 *          the previous poll is released either way). On return
 *          `size` holds how many bytes were filled, which is stable across
 *          a loop that reuses one struct.
 *          KuiEvent, KuiDrawData, KuiTextMetrics, KuiScrollGeometry,
 *          KuiWindowCommand.
 *
 * [out[]]  You allocate an ARRAY; the library fills up to `cap` elements.
 *          A `size` field cannot help here: the library strides by its own
 *          sizeof, so element 1 lands past your element 1 no matter what
 *          element 0 says, and the write happens before any in-band
 *          handshake could be read. If one of these ever has to grow, it
 *          grows by gaining an explicit stride argument - a source break
 *          every host sees - and by bumping KUI_ABI_VERSION. Until then the
 *          version check is the whole guard.
 *          KuiAccessNode, KuiAccessRun, KuiWarning, KuiAudioCommand.
 *
 * [lib]    The library allocates it; you read it. KuiQuad, through
 *          KuiDrawData.quads. The same stride problem mirrored - you walk
 *          the array with your sizeof - except read-only, so a mismatch
 *          misreads every quad after the first rather than corrupting
 *          anything. Guarded by KUI_ABI_VERSION.
 *
 * KuiStr is the exception and is frozen: it crosses both ways (kui_edit_text
 * and kui_value_as_str write one) and will never be more than (ptr, len).
 * KuiEvent also reaches kui_run's callback as a library-owned
 * `const KuiEvent *`; one struct behind a pointer is safe to append to, so
 * it is the [out] use above that pins the type.
 */

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
/* KUI_QUAD_SHADOW: `color` fills a rounded rect inset from the quad by
   `blur` on every side, its edge ramped over `blur` px; ignores uv,
   border_color and border_w. */
enum { KUI_QUAD_SOLID = 0, KUI_QUAD_GLYPH_MASK = 1, KUI_QUAD_GLYPH_COLOR = 2,
       KUI_QUAD_IMAGE = 3, KUI_QUAD_GLYPH_SUBPIXEL = 4, KUI_QUAD_SHADOW = 5 };
/* Font families (KuiTextStyle.family) */
enum { KUI_FONT_SANS = 0, KUI_FONT_SERIF = 1, KUI_FONT_MONO = 2 };
/* Line breaking (KuiTextStyle.wrap) */
enum { KUI_WRAP_WORD = 0, KUI_WRAP_GLYPH = 1, KUI_WRAP_NONE = 2 };
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
/* Float modes (KuiSpec.float_mode). For the named presets the other
 * bindings take ("below", "above", ...), see kui_spec_float_preset. */
enum { KUI_FLOAT_NONE = 0, KUI_FLOAT_PARENT = 1, KUI_FLOAT_VIEWPORT = 2 };
/* Window-chrome roles (KuiSpec.window_role). Chrome nodes turn input into
 * window commands (kui_take_window_command), never events. */
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
/* Window commands (KuiWindowCommand.kind, kui_take_window_command). The
 * first four are what chrome nodes ask for, about the window they were
 * drawn in; KUI_CMD_OPEN and KUI_CMD_CLOSE are also what the declared
 * window set's diff decides (kui_window_declare). The last two are what
 * the app itself asked for (kui_set_window_size / kui_focus_window). */
enum {
    KUI_CMD_START_DRAG = 1,
    KUI_CMD_CLOSE = 2,
    KUI_CMD_MINIMIZE = 3,
    KUI_CMD_TOGGLE_MAXIMIZE = 4,
    KUI_CMD_OPEN = 5,
    KUI_CMD_SET_SIZE = 6,
    KUI_CMD_FOCUS = 7,
};

/* [in] */
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
    KUI_KF_OPACITY = 1u << 5,
};

/* One CSS-style keyframe stop. A zeroed stop sets nothing: `set` says which
 * fields count, so 0 stays a legal value for each. Stops without KUI_KF_AT
 * spread evenly (a lone stop sits at 1 and animates from the node's own
 * value); declared `at`s must not decrease. Sizings animate their amount
 * only, in the form the spec's own width/height declares. */
/* [in] */
typedef struct KuiKeyframe {
    uint32_t set;
    float at; /* 0..1 */
    KuiSizing width, height;
    uint32_t bg; /* 0xRRGGBBAA */
    float radius;
    float opacity; /* group opacity 0..1 */
} KuiKeyframe;

/* Which KuiEnter fields are set (KuiEnter.set bits); 0 = no entrance. */
enum {
    KUI_ENTER_OFFSET = 1u << 0,
    KUI_ENTER_WIDTH = 1u << 1,
    KUI_ENTER_HEIGHT = 1u << 2,
    KUI_ENTER_BG = 1u << 3,
    KUI_ENTER_RADIUS = 1u << 4,
    KUI_ENTER_OPACITY = 1u << 5,
};

/* Where a node starts the first frame it is seen (KuiSpec.enter): the slots
 * `set` names ease in from these values over transition_ms instead of
 * snapping — dx/dy slide it in from that far away (logical px), bg fades
 * the node and opacity the whole subtree in. A node drawn again after a
 * frame away enters again. Zeroed = none. */
/* [in] */
typedef struct KuiEnter {
    uint32_t set;
    float dx, dy;
    KuiSizing width, height;
    uint32_t bg; /* 0xRRGGBBAA */
    float radius;
    float opacity; /* group opacity 0..1; 0 fades the subtree in */
} KuiEnter;

/* [in] Zero-initialized KuiSpec is a fit-sized transparent column. Colors
 * are 0xRRGGBBAA with 0 meaning "none". Fields mirror the shared prop schema
 * (crates/kui-core/src/schema.rs) and are append-only, which is safe here
 * precisely because the library only reads this one: see "Who writes what".
 * `tooltip` was appended that way, and did not bump KUI_ABI_VERSION. */
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
     * Attach points use KUI_START/CENTER/END; dx/dy is a logical-px offset.
     * kui_spec_float_preset fills all of these from a preset name. */
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
    /* Entrance transition (see KuiEnter); `set` = 0 leaves first sight
     * snapping as before. Needs transition_ms (defaults to 200 if 0) and a
     * stable key; with `slide` the position keeps easing afterwards, without
     * it only the entrance moves. */
    KuiEnter enter;
    /* Registered sounds (kui_sound_add) played when the node is clicked /
     * the pointer enters it; 0 = none. Either makes the node hover-tracked. */
    uint64_t click_sound;
    uint64_t hover_sound;
    /* Layout tag (NULL = none): the rect layout gave the node arrives as
     * {kind="layout", x, y, w, h, parent={x,y,w,h}, tag} (logical px,
     * viewport coords) on its first frame and whenever it changes — never
     * on a frame that left it alone. Borrowed: cloned while the node opens,
     * so you keep ownership; kui_value_null() asks for untagged events.
     * Needs a stable key (kui_open_keyed). */
    const KuiValue *on_layout;
    /* Accessibility (docs/adr/0001-accessibility-as-data.md). role: KUI_ROLE_*
     * (0 = unset: the core derives one — an on_click node is a button, an
     * editor a text input, a scrolling box a scroll view, a plain box
     * nothing; KUI_ROLE_NONE hides the node and its subtree). label: the
     * accessible name (empty = none; a button, link, tab or heading is then
     * named by the text inside it, an image or icon button has no name and
     * the core warns). checked: a checkbox / radio / switch role's on
     * state (selected / expanded are below). value_*: a slider role's position and range, each present
     * when its KUI_VALUE_* bit is in value_set. */
    uint32_t role;
    KuiStr label;
    uint32_t checked;
    uint32_t value_set;
    float value_now, value_min, value_max;
    /* On a KUI_ROLE_LINE of a custom editor (an on_key sink with a
     * KUI_ROLE_TEXT_INPUT / MULTILINE_TEXT_INPUT role that draws its own
     * text): the caret's byte offset into the line's text, and the byte
     * offset of the selection's other end; present when KUI_VALUE_CARET /
     * KUI_VALUE_ANCHOR are in value_set. */
    uint32_t caret;
    uint32_t selection_anchor;
    /* Keyboard focus (docs/adr/0002-keyboard-focus-as-data.md). focusable:
     * non-zero puts the node in the Tab ring (and a click focuses it)
     * without a click payload or a control role; editors, on_key sinks,
     * on_click nodes and the control roles are focusable already.
     * disabled: non-zero makes the node inert — no click, drag or key sink,
     * no hover / pressed / focus background, skipped by Tab, reported to
     * assistive technology (hover tracking stays so a tooltip can say why).
     * focus_bg: 0xRRGGBBAA while the node holds keyboard-visible focus (Tab
     * or assistive technology put it there); 0 = the core's default ring. */
    uint32_t focusable;
    uint32_t disabled;
    uint32_t focus_bg;
    /* Hover hint (empty = none), the `tooltip` prop of the other bindings:
     * makes the node hover-tracked, becomes its accessible description,
     * and floats the hint below it while the pointer is over it. kui_close
     * draws that float, so this only applies to nodes opened with the
     * kui_open* family; for a hint that always draws, or one around custom
     * content, call kui_tooltip / kui_tooltip_with yourself. Borrowed
     * while the node opens. */
    KuiStr tooltip;
    /* Modal surface, NULL = none (docs/adr/0003-modal-surfaces.md): while
     * this node is declared the Tab ring is its subtree, everything
     * outside it is inert to the pointer, the wheel and assistive
     * technology (window chrome stays live), and Escape or a press
     * outside emits {kind="dismiss", reason="escape"|"outside", tag} on
     * it — the core closes nothing, the app stops opening the node. The
     * last one declared in tree order is the one in effect (a confirm
     * inside a dialog); a modal that must cover the app is a float.
     * Borrowed: cloned while the node opens, so you keep ownership;
     * kui_value_null() asks for the behaviour without a tag. */
    const KuiValue *modal;
    /* Context menu, NULL = none: a secondary-button press over this node
     * emits {kind="contextmenu", x, y, tag} on it, at the logical point to
     * open the menu at. The press does nothing else — it moves no focus,
     * places no caret and produces no click, so right-clicking a selection
     * keeps it. Routed like a click: the topmost node under the pointer is
     * the one asked. Borrowed: cloned while the node opens, so you keep
     * ownership; kui_value_null() asks for the behaviour without a tag. */
    const KuiValue *on_context_menu;
    /* KUI_CURSOR_* (0 = unset: the core derives one). Overrides the
     * pointer shape while the pointer is over this node — the core
     * otherwise makes an editor a caret, an on_click / focusable node a
     * hand, an on_drag node a grab, and everything else the arrow. For
     * what that cannot know: a splitter (KUI_CURSOR_EW_RESIZE), a
     * disabled control that says KUI_CURSOR_NOT_ALLOWED. A node with
     * nothing but a cursor is hover-tracked so it can be found. */
    uint32_t cursor;
    /* Selection and disclosure. selected: non-zero when this node is the
     * current one of its set - the shown tab, the picked row, the link for
     * the page you are on. A KUI_ROLE_TAB reports the state either way (its
     * siblings read as "not selected"); a row or a link reports it only
     * where this is set, since an ordinary list or navigation bar is not a
     * selection. expanded: KUI_EXPANDED_* for a node that shows and hides
     * something (0 = unset: it does not expand, and a reader says nothing
     * about it). "3 of 7" is not declared - the core numbers a
     * KUI_ROLE_LIST's rows and a KUI_ROLE_TAB_LIST's tabs itself, and
     * reports them in KuiAccessNode.pos_in_set / set_size. */
    uint32_t selected;
    uint32_t expanded;
    /* Group opacity: with opacity_set non-zero, `opacity` (0..1) fades this
     * node and its whole subtree. The bit exists so 0 stays expressible —
     * without it, a zeroed struct could not tell "opaque" from "invisible".
     * It is a per-quad alpha multiply, not an offscreen composite, so
     * overlapping pieces of one subtree show their seams through the fade.
     * Layout, hit-testing and the access tree are untouched: an invisible
     * subtree still takes clicks, exactly like CSS opacity: 0. Eases with
     * transition_ms, and enter.opacity fades a panel in. */
    uint32_t opacity_set;
    float opacity;
    /* Drop shadow: the node's rounded rect, moved by shadow_x/shadow_y,
     * grown by shadow_spread and blurred over shadow_blur, painted in
     * shadow_color behind the node (CSS box-shadow without the inset and
     * multi-shadow forms). 0xRRGGBBAA with 0 = no shadow: nothing else here
     * draws without a color, and a color on its own is a hard shadow
     * exactly behind the node. Outer shadows only, and the shape is not
     * knocked out of the middle, so a translucent bg shows it through.
     * All four numbers are logical px and ease with transition_ms. */
    uint32_t shadow_color;
    float shadow_blur;
    float shadow_x, shadow_y;
    float shadow_spread;
    /* Non-zero: children that do not fit the main-axis content box start a
     * new line instead of overflowing it (or shrinking to fit) — a tag
     * list, a chip toolbar, a button row that reflows when the window
     * narrows. cross_gap is the space between the lines; gap stays the
     * space between children along one. Rows only: a column, or a row with
     * KUI_OVERFLOW_SCROLL_X, lays out as if this were 0 and raises a
     * "wrap-ignored" warning (kui_take_warnings). */
    uint32_t wrap_children;
    float cross_gap;
    /* Non-zero: where focus lands when the modal scope containing this
     * node is entered - the first node in the modal's Tab ring declaring
     * it, instead of the ring's first, so a destructive confirm opens on
     * its Cancel rather than on whichever control comes first
     * (docs/adr/0003-modal-surfaces.md). Read on entry only: a Tab press
     * afterwards stands, and the scope re-entered (a nested confirm
     * closing) leaves focus where it was. A node the ring skips
     * (disabled, KUI_ROLE_NONE, not focusable) is not a candidate, and
     * with no candidate the entry is the ring's first node as before. */
    uint32_t initial_focus;
    /* Exit transition (see KuiEnter, which an exit reuses — an exit is an
     * entrance read the other way). With `set` non-zero and a transition_ms,
     * the frame after the view stops declaring this node its subtree is
     * copied out of the last frame that had it and replayed: frozen where
     * layout left it, painted on top of everything and outside every clip,
     * and inert — no clicks, no Tab stop, no access row — while the slots
     * `set` names ease from where they were to these values. Then it is
     * dropped; so is a ghost whose key the view declares again, so a toast
     * dismissed and re-shown never doubles. `set` = 0, or no transition_ms,
     * leaves a removed node vanishing at once as before. Needs a stable key.
     * width/height resize the departing node's own box only: what is inside
     * it is a picture and is not laid out again. More than 512 nodes
     * departing at once is refused past the budget (the rest vanish) with an
     * "exit-budget" warning. */
    KuiEnter exit;
    /* KUI_LIVE_*: when the text inside this node changes, a screen reader
     * reads the change without being asked. A node that declares it is
     * semantic, so a plain box marked live is not elided from the access
     * tree; put it on the smallest node holding the message, since
     * everything inside a live node is live. For a one-off with no node
     * behind it, kui_announce is the other half. See
     * docs/adr/0008-live-regions-and-announcements.md. */
    uint32_t live;
} KuiSpec;

/* Disclosure state (KuiSpec.expanded): the schema index plus one, so zero
 * can mean "this node does not expand". */
enum {
    KUI_EXPANDED_COLLAPSED = 1,
    KUI_EXPANDED_EXPANDED = 2,
};

/* Live-region politeness (KuiSpec.live, KuiAnnouncement.live): the schema
 * index itself, not the index plus one — unlike a disclosure, "not a live
 * region" is what a zeroed field already means, so there is no unset state
 * to reserve zero for. */
enum {
    KUI_LIVE_OFF = 0,
    KUI_LIVE_POLITE = 1,
    KUI_LIVE_ASSERTIVE = 2,
};

/* Pointer shapes (KuiSpec.cursor), and what kui_cursor_shape answers with. */
enum {
    KUI_CURSOR_DEFAULT = 1, KUI_CURSOR_TEXT, KUI_CURSOR_POINTER,
    KUI_CURSOR_GRAB, KUI_CURSOR_GRABBING, KUI_CURSOR_NOT_ALLOWED,
    KUI_CURSOR_EW_RESIZE, KUI_CURSOR_NS_RESIZE, KUI_CURSOR_NWSE_RESIZE,
    KUI_CURSOR_NESW_RESIZE,
};

/* Roles (KuiSpec.role, KuiAccessNode.role). Declarable on any node:
 * KUI_ROLE_NONE through KUI_ROLE_GROUP, plus KUI_ROLE_TEXT_INPUT,
 * KUI_ROLE_MULTILINE_TEXT_INPUT and KUI_ROLE_LINE (which an app that
 * draws its own text declares to make a key sink an editor and to mark
 * that editor's lines) and the three ADR 0007 appended,
 * KUI_ROLE_RADIO_GROUP through KUI_ROLE_MENU_ITEM. A role can only be
 * appended (KUI_ROLE_* is the position in Rust's Role::ALL plus one, and
 * the Lua and Node wires carry the same index), so what is declarable is
 * a list rather than a range. The rest the core derives from what a node
 * is.
 *
 * KUI_ROLE_RADIO_GROUP, KUI_ROLE_TAB_LIST, KUI_ROLE_MENU and KUI_ROLE_LIST
 * are the composite containers: one holding focusable KUI_ROLE_RADIO,
 * KUI_ROLE_TAB, KUI_ROLE_MENU_ITEM or KUI_ROLE_LIST_ITEM children is a
 * single Tab stop with the arrow keys moving inside it
 * (docs/adr/0007-composite-keyboard-patterns.md). Nothing declares that:
 * the core derives it from the roles and from which nodes are focusable. */
enum {
    KUI_ROLE_NONE = 1, KUI_ROLE_BUTTON, KUI_ROLE_CHECKBOX, KUI_ROLE_RADIO,
    KUI_ROLE_SWITCH, KUI_ROLE_SLIDER, KUI_ROLE_TAB, KUI_ROLE_TAB_LIST,
    KUI_ROLE_LINK, KUI_ROLE_HEADING, KUI_ROLE_LIST, KUI_ROLE_LIST_ITEM,
    KUI_ROLE_IMAGE, KUI_ROLE_DIALOG, KUI_ROLE_GROUP,
    KUI_ROLE_WINDOW, KUI_ROLE_TITLE_BAR, KUI_ROLE_STATIC_TEXT,
    KUI_ROLE_TEXT_INPUT, KUI_ROLE_MULTILINE_TEXT_INPUT, KUI_ROLE_SCROLL_VIEW,
    KUI_ROLE_LINE,
    KUI_ROLE_RADIO_GROUP, KUI_ROLE_MENU, KUI_ROLE_MENU_ITEM,
};

/* How a composite container arranges its items
 * (KuiAccessNode.orientation, 0 = unset). Derived from the container's
 * own KuiSpec.dir and never declared: the layout is what arranges the
 * items. An announcement, not a gate — both arrow pairs move inside a
 * composite whatever this says. */
enum {
    KUI_ORIENTATION_HORIZONTAL = 1,
    KUI_ORIENTATION_VERTICAL = 2,
};
/* Which of KuiSpec.value_now / value_min / value_max / caret /
 * selection_anchor are set */
enum {
    KUI_VALUE_NOW = 1u << 0,
    KUI_VALUE_MIN = 1u << 1,
    KUI_VALUE_MAX = 1u << 2,
    KUI_VALUE_CARET = 1u << 3,
    KUI_VALUE_ANCHOR = 1u << 4,
};
/* Actions assistive technology can request (KuiAccessNode.actions bits,
 * kui_input_access). */
enum {
    KUI_ACCESS_CLICK = 1u << 0,
    KUI_ACCESS_FOCUS = 1u << 1,
    KUI_ACCESS_BLUR = 1u << 2,
    KUI_ACCESS_SET_VALUE = 1u << 3,
    KUI_ACCESS_INCREMENT = 1u << 4,
    KUI_ACCESS_DECREMENT = 1u << 5,
    KUI_ACCESS_SCROLL_INTO_VIEW = 1u << 6,
    KUI_ACCESS_SCROLL_UP = 1u << 7,
    KUI_ACCESS_SCROLL_DOWN = 1u << 8,
    KUI_ACCESS_SCROLL_LEFT = 1u << 9,
    KUI_ACCESS_SCROLL_RIGHT = 1u << 10,
    KUI_ACCESS_SET_TEXT_SELECTION = 1u << 11,
    KUI_ACCESS_REPLACE_SELECTED_TEXT = 1u << 12,
};
/* KuiAccessNode.flags: which optional fields hold, and state. */
enum {
    KUI_ACCESS_HAS_VALUE = 1u << 0,
    KUI_ACCESS_HAS_SELECTION = 1u << 1,
    KUI_ACCESS_FOCUSED = 1u << 2,
    KUI_ACCESS_CHECKED_SET = 1u << 3,
    KUI_ACCESS_CHECKED = 1u << 4,
    KUI_ACCESS_HAS_NUMBER = 1u << 5,
    KUI_ACCESS_HAS_MIN = 1u << 6,
    KUI_ACCESS_HAS_MAX = 1u << 7,
    KUI_ACCESS_HAS_SCROLL = 1u << 8,
    KUI_ACCESS_HAS_TEXT_SELECTION = 1u << 9,
    KUI_ACCESS_DISABLED = 1u << 10, /* declared disabled: inert, not a Tab stop */
    KUI_ACCESS_MODAL = 1u << 11,    /* the frame's modal surface (aria-modal) */
    /* Whether the node has a selected state at all, and what it is. */
    KUI_ACCESS_SELECTED_SET = 1u << 12,
    KUI_ACCESS_SELECTED = 1u << 13,
    /* Whether the node expands, and whether it is open. */
    KUI_ACCESS_EXPANDED_SET = 1u << 14,
    KUI_ACCESS_EXPANDED = 1u << 15,
    /* pos_in_set holds (on an item), set_size holds (on its container). */
    KUI_ACCESS_HAS_POS_IN_SET = 1u << 16,
    KUI_ACCESS_HAS_SET_SIZE = 1u << 17,
    /* The node declared KuiSpec.live, and which politeness. Two flag bits
     * rather than a `live` field: KuiAccessNode is [out[]], so the host
     * allocates the array and appending to it would be an ABI break. */
    KUI_ACCESS_LIVE_POLITE = 1u << 18,
    KUI_ACCESS_LIVE_ASSERTIVE = 1u << 19,
};

/* [out[]] One queued announcement (kui_take_announcements): something to say
 * once, with no node behind it. `live` is KUI_LIVE_POLITE or
 * KUI_LIVE_ASSERTIVE, never KUI_LIVE_OFF. Strings are borrowed until the
 * next kui_take_announcements on the context.
 * See docs/adr/0008-live-regions-and-announcements.md. */
typedef struct KuiAnnouncement {
    KuiStr text;
    uint32_t live;
} KuiAnnouncement;

/* [out[]] One node of the access tree (kui_access_tree): what assistive technology
 * sees. Plain boxes are elided, so `parent` is the nearest semantic
 * ancestor (0 for the root). Rects are logical px in viewport coordinates.
 * Strings are borrowed until the next kui_access_tree on the context. */
typedef struct KuiAccessNode {
    uint64_t key;
    uint64_t parent;
    uint32_t origin;
    uint32_t role;    /* KUI_ROLE_* */
    uint32_t flags;   /* KUI_ACCESS_HAS_* / FOCUSED / CHECKED / SELECTED / EXPANDED */
    uint32_t actions; /* KUI_ACCESS_* the node accepts */
    KuiStr name;
    KuiStr description;
    KuiStr value; /* an editor's text (KUI_ACCESS_HAS_VALUE) */
    float x, y, w, h;
    uint32_t caret, selection_start, selection_end; /* byte offsets into value */
    float value_now, value_min, value_max;          /* a slider's position and range */
    float scroll_x, scroll_y, scroll_max_x, scroll_max_y; /* a scroll view's offsets */
    /* An editor's caret (focus_*) and the selection's other end (anchor_*)
     * as run positions (KUI_ACCESS_HAS_TEXT_SELECTION): a run key from
     * kui_access_runs and a character index into that run. */
    uint64_t anchor_run;
    uint32_t anchor_char;
    uint64_t focus_run;
    uint32_t focus_char;
    uint32_t run_count; /* how many runs kui_access_runs returns */
    /* "3 of 7", derived from the list or tab list holding this node: the
     * item's zero-based ordinal (KUI_ACCESS_HAS_POS_IN_SET) and, on that
     * container, how many items it holds (KUI_ACCESS_HAS_SET_SIZE). The
     * count sits on the container, not on each item, which is how
     * AccessKit models a set (ARIA repeats aria-setsize on every item). */
    uint32_t pos_in_set;
    uint32_t set_size;
    /* How a composite container arranges its items (KUI_ORIENTATION_*,
     * 0 = unset: this node is not one). Derived from its own dir. */
    uint32_t orientation;
} KuiAccessNode;

/* [out[]] One laid-out run of an editor's text (kui_access_runs): what a screen
 * reader reads by character and word. `text` ends with "\n" (a character
 * of zero width) when the line continues into another; `line` is a
 * buffer line (a KUI_ROLE_LINE ordinal for a custom editor) and
 * start/end the run's byte range in that line's text. Character positions
 * are relative to x. Arrays and strings are borrowed until the next
 * kui_access_tree / kui_access_runs on the context. */
typedef struct KuiAccessRun {
    uint64_t key;
    uint32_t line, start, end;
    KuiStr text;
    float x, y, w, h;
    uint32_t char_count;
    const uint8_t *char_lengths;
    const float *char_positions;
    const float *char_widths;
    uint32_t word_start_count;
    const uint8_t *word_starts;
    uint32_t rtl;
} KuiAccessRun;

/* [out] What text measures (kui_measure_text): logical px at the scale of
 * the current or last frame; `lines` after wrapping. */
typedef struct KuiTextMetrics {
    uint32_t size; /* = sizeof(KuiTextMetrics) in, bytes filled out */
    float width, height;
    uint32_t lines;
} KuiTextMetrics;
#define KUI_TEXT_METRICS_INIT ((KuiTextMetrics){ .size = sizeof(KuiTextMetrics) })

/* [out] What the last layout resolved for a scroll container
 * (kui_scroll_geometry): its box, its content size and the clamped offset,
 * logical px in viewport coordinates. */
typedef struct KuiScrollGeometry {
    uint32_t size; /* = sizeof(KuiScrollGeometry) in, bytes filled out */
    float x, y, w, h;
    float content_w, content_h;
    /* Where it is scrolled to, always a position within the content: the
     * retained offset clamped to the travel below. */
    float offset_x, offset_y;
    /* How far it can travel; zero on an axis that does not scroll. */
    float max_offset_x, max_offset_y;
} KuiScrollGeometry;
#define KUI_SCROLL_GEOMETRY_INIT ((KuiScrollGeometry){ .size = sizeof(KuiScrollGeometry) })

/* [out[]] A silent misconfiguration the core noticed (kui_take_warnings). `code`
 * is stable — "grow-weight-ignored", "transition-auto-key",
 * "duplicate-key" — `key` the node it is about, `message` for people.
 * Strings are borrowed until the next kui_take_warnings on the context. */
typedef struct KuiWarning {
    KuiStr code;
    uint64_t key;
    KuiStr message;
} KuiWarning;

/* -- Audio ----------------------------------------------------------------
 * Sounds are resources, playback is commands: kui_run plays them through
 * the bundled device; a host with its own loop drains
 * kui_take_audio_commands and reports finished playbacks with
 * kui_audio_ended. Volumes are linear amplitude (0..1), durations ms. */

/* [in] Options for kui_play. NULL = defaults; a struct is read literally, so start
 * from KUI_PLAY_INIT (volume 1) rather than zero. */
typedef struct KuiPlay {
    float volume;
    uint32_t looped;
    float fade_in_ms;
} KuiPlay;
#define KUI_PLAY_INIT ((KuiPlay){ .volume = 1.0f })

/* [in] What a kui_audio node declares; read literally (start from KUI_AUDIO_INIT). */
typedef struct KuiAudio {
    uint64_t src;    /* a registered sound */
    float volume;
    uint32_t looped;
    uint32_t paused; /* holds the playback; resumes when cleared */
} KuiAudio;
#define KUI_AUDIO_INIT(id) ((KuiAudio){ .src = (id), .volume = 1.0f })

/* Audio command kinds (KuiAudioCommand.kind). */
enum {
    KUI_AUDIO_PLAY = 1,          /* playback, sound, volume, ms = fade-in, looped */
    KUI_AUDIO_STOP = 2,          /* playback, ms = fade-out */
    KUI_AUDIO_SET_VOLUME = 3,    /* playback, volume, ms = tween */
    KUI_AUDIO_PAUSE = 4,         /* playback, ms = fade-out */
    KUI_AUDIO_RESUME = 5,        /* playback, ms = fade-in */
    KUI_AUDIO_MASTER_VOLUME = 6, /* volume, ms = tween */
    KUI_AUDIO_UNLOAD = 7,        /* sound: drop any decoded copy */
};
/* [out[]] One queued command (kui_take_audio_commands). */
typedef struct KuiAudioCommand {
    uint32_t kind;
    uint64_t playback;
    uint64_t sound;
    float volume;
    float ms;
    uint32_t looped;
} KuiAudioCommand;

/* [in] Zero-initialized KuiTextStyle picks defaults (16px, default foreground). */
typedef struct KuiTextStyle {
    float size;
    float line_height; /* <= 0: default (size * 1.35) */
    uint32_t color;    /* 0: default foreground */
    uint32_t family;   /* KUI_FONT_* ; 0 = sans */
    uint64_t font;     /* registered font handle (kui_font_add*); non-zero overrides family */
    uint32_t wrap;     /* KUI_WRAP_* ; 0 = between words */
    uint32_t max_lines; /* at most this many lines; 0 = unlimited */
    uint32_t ellipsis; /* non-zero: end the last line with "..." when cut off (one line unless max_lines) */
} KuiTextStyle;

/* [in] One run of a rich-text paragraph. */
typedef struct KuiSpan {
    KuiStr text;
    uint32_t color; /* 0: inherit paragraph color */
    uint32_t flags; /* KUI_SPAN_* */
} KuiSpan;

/* The window an app starts in - the one kui_run opens - which is always
 * live and is named "main". Other windows get the id their KUI_CMD_OPEN
 * carried. */
#define KUI_WINDOW_MAIN 0u

/* What kind of surface a declared window is (KuiWindowConfig.kind). Only
 * the normal window exists: the borderless, non-activating popup is ADR
 * 0004's step 4, which this release does not ship, so there is no
 * KUI_WINDOW_KIND_POPUP to name yet. Any other value opens a normal window
 * - so a host built against a later header degrades to a window rather than
 * to nothing - and raises the unknown-window-kind warning saying so. */
#define KUI_WINDOW_KIND_NORMAL 0u

/* [in] What a declared window is (kui_window_declare), and what a
 * KUI_CMD_OPEN carries back out. Read literally, so start from
 * KUI_WINDOW_CONFIG_INIT - a normal, activating 640x480 window - or pass
 * NULL for exactly that. A zero width or height means the default. */
typedef struct KuiWindowConfig {
    uint32_t kind;      /* KUI_WINDOW_KIND_*; anything else warns and opens normal */
    float width, height; /* initial inner size, logical px */
    uint32_t activates; /* whether opening it takes OS focus */
} KuiWindowConfig;
#define KUI_WINDOW_CONFIG_INIT \
    ((KuiWindowConfig){ .kind = KUI_WINDOW_KIND_NORMAL, .width = 640, .height = 480, .activates = 1 })

/* [out] One window command (kui_take_window_command): what a chrome node
 * asked for, or what the declared window set decided. Plain data - an open
 * carries no title; the window's first frame declares one through
 * kui_window_title - so nothing borrowed enters your drain loop. */
typedef struct KuiWindowCommand {
    uint32_t size;   /* = sizeof(KuiWindowCommand) in, bytes filled out */
    uint32_t kind;   /* KUI_CMD_* */
    uint32_t window; /* which window; for KUI_CMD_OPEN the new window's id */
    uint16_t origin; /* KUI_CMD_OPEN: whose declaration won (0 = you, 1+ = an extension) */
    KuiWindowConfig config; /* KUI_CMD_OPEN only */
    float width, height;    /* KUI_CMD_SET_SIZE only: the size asked for, logical px */
} KuiWindowCommand;
#define KUI_WINDOW_COMMAND_INIT ((KuiWindowCommand){ .size = sizeof(KuiWindowCommand) })

/* [out] One polled event. `size` leads it so that a field appended later
 * reaches a host that has not recompiled as a shorter write, not a longer
 * one - `window` is the first field that actually did (ABI 4). A host built
 * against ABI 3 reserves through `payload`, which is still the ABI-1 floor,
 * so it keeps polling correctly and simply never sees `window`. */
typedef struct KuiEvent {
    uint32_t size;           /* = sizeof(KuiEvent) in, bytes filled out */
    uint16_t origin;         /* which frontend drew the node: 0 = you, 1+ = extensions */
    uint64_t key;
    const KuiValue *payload; /* borrowed; may be NULL */
    uint32_t window;         /* which window it came from: the KuiCtx's kui_env_set_window id */
} KuiEvent;
#define KUI_EVENT_INIT ((KuiEvent){ .size = sizeof(KuiEvent) })

/* [lib] One quad of the display list, read through KuiDrawData.quads. You
 * stride the array with your own sizeof, so its layout is pinned by
 * KUI_ABI_VERSION rather than by anything in-band. */
typedef struct KuiQuad {
    float x, y, w, h;        /* physical pixels */
    float color[4];
    float border_color[4];
    float radius[4];         /* corner radii, clockwise from the top-left */
    float border_w;
    float blur;              /* KUI_QUAD_SHADOW: blur radius, also how far the rect is inflated */
    uint32_t kind;           /* KUI_QUAD_* */
    uint32_t uv[4];          /* atlas texels: x, y, w, h */
    float clip[4];           /* clip rect (physical px): pixels outside are transparent */
    /* Corner radii of the clip (physical px), clockwise from the top-left:
     * pixels outside the ROUNDED clip are transparent too. A clipping node
     * with a radius rounds what it clips, the way CSS rounds
     * `overflow: hidden` under a `border-radius`. All zero - every quad of
     * a frame with no rounded clipper - is the plain rect clip, so a
     * renderer that ignores this field is correct until an app rounds one. */
    float clip_radius[4];
} KuiQuad;

/* [out] Everything a renderer needs for the finished frame. */
typedef struct KuiDrawData {
    uint32_t size;                /* = sizeof(KuiDrawData) in, bytes filled out */
    const KuiQuad *quads;
    size_t quad_count;
    float viewport_w, viewport_h; /* physical pixels */
    float scale;
    const uint8_t *atlas_pixels;  /* RGBA, atlas_size^2 * 4 bytes */
    uint32_t atlas_size;
    bool atlas_dirty;             /* re-upload when set or epoch changed */
    uint64_t atlas_epoch;
} KuiDrawData;
#define KUI_DRAW_DATA_INIT ((KuiDrawData){ .size = sizeof(KuiDrawData) })

/* -- Context ------------------------------------------------------------- */
KuiCtx *kui_ctx_new(void);
void kui_ctx_free(KuiCtx *ctx);

/* -- Input (logical coordinates) + events -------------------------------- */
void kui_input_cursor(KuiCtx *ctx, float x, float y);
void kui_input_cursor_left(KuiCtx *ctx);
/* clicks: host-counted multi-click for presses (1 single, 2 double = word
 * select in editors, 3 triple = line select); ignored on release. This is
 * the primary button; kui_input_mouse_button carries the others. */
void kui_input_mouse(KuiCtx *ctx, bool down, uint32_t clicks);
/* Buttons (kui_input_mouse_button). Only the primary one presses, drags,
 * places the caret and clicks; the secondary one asks the node under it
 * for a context menu (KuiSpec.on_context_menu) and moves nothing else.
 * Nothing routes the rest yet; pass 3 + n for a further button n so a
 * driver need not drop it. */
enum {
    KUI_MOUSE_PRIMARY = 0,
    KUI_MOUSE_SECONDARY = 1,
    KUI_MOUSE_MIDDLE = 2,
    KUI_MOUSE_OTHER = 3,
};
/* kui_input_mouse for a named button (KUI_MOUSE_*). */
void kui_input_mouse_button(KuiCtx *ctx, bool down, uint32_t button,
                            uint32_t clicks);
void kui_input_scroll(KuiCtx *ctx, float dx, float dy); /* +y = scroll up */
void kui_input_text(KuiCtx *ctx, KuiStr text);   /* typing/paste -> focused editor */
/* In-progress IME composition shown at the focused caret; empty text clears
 * it, the commit arrives via kui_input_text. cursor_* are byte offsets into
 * text (UINT32_MAX = none). */
void kui_input_preedit(KuiCtx *ctx, KuiStr text, uint32_t cursor_start,
                       uint32_t cursor_end);
void kui_input_key(KuiCtx *ctx, uint32_t key, uint32_t mods); /* KUI_KEY_* + KUI_MOD_* */
/* Raw keys for on_key sinks (the editing keys go through kui_input_key
 * above). `code` is a single character as the layout produced it ("W", "$")
 * or a name ("left", "enter", "escape", "f5", ...); `physical` is the
 * US-QWERTY key at that *position*, spelled the same way, or {NULL, 0} when
 * the host does not track positions (then it equals `code`); `kmods` is
 * KUI_KMOD_* bits; `text` is what the press inserts, or {NULL, 0} to derive
 * it from `code`; `repeat` marks an auto-repeat. The focused sink polls
 * {kind="key", phase="down"|"up", code, physical, ctrl, alt, shift, super,
 * text, repeat, tag}; a release carries a null `text`. A release whose press
 * the sink never got resolves nothing, and moving focus while a key is held
 * delivers the "up" first, so a held-key binding (WASD, press-and-hold)
 * cannot be left stuck down. An unknown `code` or `physical` is ignored.
 *
 * Passing both is what makes a keymap portable. A layout that produces
 * something outside ASCII (Cyrillic, Greek, Hebrew, Arabic) would leave a
 * Latin keymap matching nothing at all, so kui reports the position's US
 * letter as `code` instead; `physical` is there either way for a keymap that
 * would rather bind the finger than the label (WASD). A host passing
 * {NULL, 0} keeps the old behaviour exactly. */
void kui_input_key_down(KuiCtx *ctx, KuiStr code, KuiStr physical,
                        uint32_t kmods, KuiStr text, bool repeat);
void kui_input_key_up(KuiCtx *ctx, KuiStr code, KuiStr physical,
                      uint32_t kmods);
/* Lets go of every key the focused sink is holding, as if the user had
 * released them. Call it when the window loses the keyboard: the OS stops
 * delivering key events to it, so the release of anything held over an app
 * switch would never arrive. Focus moves do this by themselves. */
void kui_release_held_keys(KuiCtx *ctx);
/* Physical modifier state changed (KUI_KMOD_* bits); the host polls a
 * {kind="modifiers", shift, ctrl, alt, super} event when it differs. */
void kui_input_modifiers(KuiCtx *ctx, uint32_t mods);
bool kui_poll_event(KuiCtx *ctx, KuiEvent *out);

/* -- Host environment ---------------------------------------------------- */
/* These two setters are the whole of C's `env`: the fields of kui_core's
 * Env and WindowEnv, one argument each, in the order docs/props.md's Env
 * table lists them (schema::ENV_FIELDS, the one statement of the shape
 * every binding's reading is pinned to). A C host is the frame driver, so
 * it writes the facts and has no reading of them back - Rust's ui.env(),
 * Lua's view(env) and Node's ctx.env() are the readers - except the window
 * id, which kui_ctx_window answers. The two facts Lua and Node derive or
 * carry beside these (the frame budget, the viewport) are the host's own
 * numbers here. kui-ffi's tests hold these prototypes to the table:
 *   kui_env_set         refresh_hz, focused          (Env)
 *   kui_env_set_window  window, custom_chrome, maximized, fullscreen,
 *                       controls_w, controls_h       (WindowEnv; the
 *                       controls rect flattened to its extent at the
 *                       window origin, the shape Lua also reads)
 */
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
/* Window facts for views (widgets adapt to them). `window` is which window
 * this context draws - KUI_WINDOW_MAIN, or the id a KUI_CMD_OPEN carried -
 * and every KuiEvent it hands out says so. controls_w/h > 0 describe the
 * top-left keep-out rect of OS-drawn controls (macOS traffic lights under
 * custom chrome). */
void kui_env_set_window(KuiCtx *ctx, uint32_t window, bool custom_chrome,
                        bool maximized, bool fullscreen, float controls_w,
                        float controls_h);
/* Pops the next window command into out: what chrome nodes asked for since
 * the last drain, and the KUI_CMD_OPEN / KUI_CMD_CLOSE the declared window
 * set decided at the last kui_frame_finish. Returns false, writing nothing,
 * when there is none - or when out->size is below the layout this library
 * knows (start from KUI_WINDOW_COMMAND_INIT), leaving the command queued.
 * Call after each input dispatch and each frame until it returns false:
 *
 *     KuiWindowCommand cmd = KUI_WINDOW_COMMAND_INIT;
 *     while (kui_take_window_command(ctx, &cmd)) {
 *         switch (cmd.kind) {
 *         case KUI_CMD_OPEN:  open a window for cmd.window with cmd.config; break;
 *         case KUI_CMD_CLOSE: close window cmd.window (exit if KUI_WINDOW_MAIN); break;
 *         case KUI_CMD_SET_SIZE: resize cmd.window to cmd.width x cmd.height; break;
 *         ...
 *         }
 *     }
 */
bool kui_take_window_command(KuiCtx *ctx, KuiWindowCommand *out);
/* Declares that a window named `name` exists this frame
 * (docs/adr/0004-multi-window.md). It opens on the first frame any window's
 * frame declares it - cfg is read then and never again (NULL means
 * KUI_WINDOW_CONFIG_INIT), because the user owns a window's geometry once
 * it exists - and closes on the first frame none does. The KUI_CMD_OPEN /
 * KUI_CMD_CLOSE arrive through kui_take_window_command, and the app sees
 * {kind:"window", phase:"opened"|"closed", name, id} through
 * kui_poll_event. A window the user closed (kui_window_closed) stays closed
 * while it is still declared - stop declaring it, then declare it again -
 * and says so with the "window-declared-while-closed" warning. Two
 * declarations of one name that disagree on the frame it opens warn
 * "duplicate-window-config"; the lowest declaring window's first one
 * wins. Call between kui_frame_begin and kui_frame_finish. */
void kui_window_declare(KuiCtx *ctx, KuiStr name, const KuiWindowConfig *cfg);
/* Asks the driver to resize a window to w x h logical px, or to give it
 * keyboard focus. Requests, not declarations: kui_window_declare's config is
 * read on the opening edge only, because the user owns a window's size once
 * it exists, so these are the only way an app moves a live window. They are
 * queued the way kui_reveal queues a scroll and come back out of your own
 * kui_take_window_command - KUI_CMD_SET_SIZE, carrying the size in
 * cmd.width/cmd.height, and KUI_CMD_FOCUS - for you to apply; a headless
 * host that never drains ignores them. window is the id events carry
 * (KuiEvent.window), KUI_WINDOW_MAIN for the launcher's. The size the window
 * actually becomes arrives as the ordinary resize event, and whether focus
 * was granted through kui_env_set's focused - neither is a reply here. */
void kui_set_window_size(KuiCtx *ctx, uint32_t window, float w, float h);
void kui_focus_window(KuiCtx *ctx, uint32_t window);
/* Which window this context draws (what kui_env_set_window set;
 * KUI_WINDOW_MAIN until then). In a kui_run view callback: the window
 * being drawn. */
uint32_t kui_ctx_window(KuiCtx *ctx);
/* Its name: "main" for the launcher's, else the name it was declared
 * under. Borrowed until the next call on the same context. */
bool kui_ctx_window_name(KuiCtx *ctx, KuiStr *out);
/* You report that the OS closed window `id` (its close button, the window
 * manager). It stays closed while still declared, whatever only it declared
 * closes with it, and the app gets {kind:"window", phase:"closed"}. Nothing
 * happens for KUI_WINDOW_MAIN or for a window already closed by the diff. */
void kui_window_closed(KuiCtx *ctx, uint32_t id);
/* The pointer shape for where the pointer is now (KUI_CURSOR_*, 0 only on a
 * bad context): derived from the topmost node under it, or its `cursor`
 * override. A state, not a queue — read after each input dispatch and each
 * frame, and set the real cursor when the answer changes. */
uint32_t kui_cursor_shape(KuiCtx *ctx);
/* Declares this frame's window title (cleared each kui_frame_begin). */
void kui_window_title(KuiCtx *ctx, KuiStr title);
/* The title declared this frame, if any — diff and apply after
 * kui_frame_finish. The view is valid until the next kui_frame_begin. */
bool kui_window_title_get(KuiCtx *ctx, KuiStr *out);

/* -- Spec helpers -------------------------------------------------------- */
/* Fills spec->float_* from a preset name — the same four the JSX and Lua
 * `float` props take ("parent", "viewport", "below", "above"), resolved by
 * the same function in kui-core, so "below" cannot mean one thing here and
 * another there. Returns false and leaves the spec alone for an unknown
 * name. The fields stay writable, so a preset is a starting point:
 *
 *     KuiSpec s = {0};
 *     kui_spec_float_preset(&s, KUI_STR("below"));
 *     s.float_dy = 12.0f;  // same attachment, a wider gap
 */
bool kui_spec_float_preset(KuiSpec *spec, KuiStr name);

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
 * plain hover colors belong in KuiSpec.hover_bg / pressed_bg. A tag of
 * kui_value_null() keeps the behaviour and leaves `tag` off the events.
 * Layout events come from KuiSpec.on_layout, not an argument. */
uint64_t kui_open_with(KuiCtx *ctx, KuiStr label, const KuiSpec *spec,
                       KuiValue *on_click, KuiValue *on_drag, KuiValue *on_key,
                       KuiValue *on_hover);
/* -- Keyboard focus (docs/adr/0002-keyboard-focus-as-data.md) ------------ */
/* One focus for every node: editors, on_key sinks, on_click nodes, the
 * control roles and `focusable` boxes are Tab stops in tree order; Enter
 * and Space press the focused control, the arrows nudge a focused slider
 * (increment / decrement access events), a key sink keeps every key (Tab
 * included) and hands focus on with kui_focus_next. Keyboard focus draws a
 * ring (or the node's focus_bg); a click's does not. */
/* Declares key focused this frame (0 blurs at once). Edge-triggered: the
 * node takes focus on the first frame it is declared, and a declaration
 * repeated every frame does not clobber a Tab press or a click. */
void kui_set_key_focus(KuiCtx *ctx, uint64_t key);
/* Moves focus to key now (0 blurs). */
void kui_focus(KuiCtx *ctx, uint64_t key);
/* What Tab (forward) / Shift-Tab does: the next / previous focusable node,
 * wrapping. */
void kui_focus_next(KuiCtx *ctx, bool forward);
/* The focused node's key, 0 for none; and whether the focus shows (it got
 * there by keyboard or assistive technology, not a click). */
uint64_t kui_focused(KuiCtx *ctx);
bool kui_focus_visible(KuiCtx *ctx);
/* -- Scrolling ------------------------------------------------------------ */
/* Scroll offsets are retained per node key and clamped by each layout to
 * that frame's overflow. The wheel, the scrollbars, Tab and the caret move
 * them from inside; these move them from outside. */
/* Scrolls whatever contains key so the node shows — what Tab does to the
 * control it lands on, asked for by name. Already-visible nodes stay put.
 * The request resolves at the next kui_frame_finish, against the frame it
 * lays out: the one being built when called from inside a view callback,
 * the one after it otherwise (a frame is requested, so one comes). That is
 * what lets a view reveal a row it is declaring for the first time. If that
 * frame does not declare key, or nothing above it scrolls, it is a no-op —
 * the request is spent, not kept for a later frame. Last reveal before a
 * frame wins. */
void kui_reveal(KuiCtx *ctx, uint64_t key);
/* Sets a scroll container's offset the way the wheel would (positive =
 * content moved up / left); the next frame's layout clamps it, so 0,0 is
 * "jump to the top" and a huge y is "jump to the end" without knowing the
 * content height. Harmless for a key that never scrolls. */
void kui_set_scroll(KuiCtx *ctx, uint64_t key, float x, float y);
/* Reads it back as the last layout clamped it — the number to persist and
 * restore. 0,0 for a node that never scrolled; either pointer may be NULL. */
void kui_scroll_offset(KuiCtx *ctx, uint64_t key, float *x, float *y);
/* Everything the last layout resolved for a container: its box, its content
 * size and that offset. False (leaving out untouched) for a key no layout
 * has resolved as a scroll container.
 *
 * This is what makes a long list affordable. The core builds every child a
 * view declares, so ten thousand rows cost ten thousand rows; a view that
 * knows h and offset_y declares the rows that fit plus two spacers holding
 * the space of the rest, and pays for a screenful. Read while building, it
 * describes the previous frame - so a resize slices one frame late, and a
 * row or two of overscan at each end covers it. */
bool kui_scroll_geometry(KuiCtx *ctx, uint64_t key, KuiScrollGeometry *out);
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
/* -- Sounds --------------------------------------------------------------- */
/* Registers a sound from its encoded file bytes (wav/ogg/mp3/flac, copied);
 * returns a handle for KuiSpec.click_sound / hover_sound, kui_audio and
 * kui_play; 0 when empty. */
uint64_t kui_sound_add(KuiCtx *ctx, const uint8_t *data, size_t len);
void kui_sound_remove(KuiCtx *ctx, uint64_t id);
/* Starts a playback; returns its id. opts may be NULL (defaults). A non-NULL
 * tag (consumed) asks for a {kind="sound", phase="ended", playback, tag}
 * event when the playback finishes on its own (never when stopped). */
uint64_t kui_play(KuiCtx *ctx, uint64_t sound, const KuiPlay *opts, KuiValue *tag);
void kui_stop(KuiCtx *ctx, uint64_t playback, float fade_ms);
void kui_set_volume(KuiCtx *ctx, uint64_t playback, float volume, float tween_ms);
void kui_pause(KuiCtx *ctx, uint64_t playback, float fade_ms);
void kui_resume(KuiCtx *ctx, uint64_t playback, float fade_ms);
void kui_set_master_volume(KuiCtx *ctx, float volume, float tween_ms);
/* An audio node: a playback retained by key while the frame declares it
 * (present = playing, once or looped; gone = stopped; volume/paused apply
 * live; a changed src restarts). Draws nothing. Empty label = a key from the
 * tree position. tag (nullable, consumed) rides the ended event. Returns the
 * node key the event carries. */
uint64_t kui_audio(KuiCtx *ctx, KuiStr label, const KuiAudio *spec, KuiValue *tag);
/* Only for hosts with their own audio device (kui_run needs neither): drains
 * queued commands into out (up to cap; the rest are dropped), returns the
 * count; report a playback that finished on its own with kui_audio_ended so
 * a tagged one becomes a sound event. */
size_t kui_take_audio_commands(KuiCtx *ctx, KuiAudioCommand *out, size_t cap);
void kui_audio_ended(KuiCtx *ctx, uint64_t playback);
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
/* -- Measurement ---------------------------------------------------------- */
/* Measures text the way layout would, without adding a node: unwrapped with
 * max_w <= 0, else wrapped to max_w logical px; the style's wrap /
 * max_lines / ellipsis apply. Works before the first frame (at scale 1).
 * Size a column to its widest label, or pick the tier that fits, from these
 * numbers instead of constants. */
bool kui_measure_text(KuiCtx *ctx, KuiStr text, const KuiTextStyle *style,
                      float max_w, KuiTextMetrics *out);
bool kui_measure_rich_text(KuiCtx *ctx, const KuiSpan *spans, size_t span_count,
                           const KuiTextStyle *base, float max_w, KuiTextMetrics *out);
/* -- Diagnostics ---------------------------------------------------------- */
/* Drains the warnings the core raised since the last call into out (up to
 * cap; the rest wait), returns the count. Each distinct (code, node) pair
 * is raised once. A host driving kui_frame_* drains them here, a test
 * asserts on them; kui_run prints them to stderr by itself in debug builds. */
size_t kui_take_warnings(KuiCtx *ctx, KuiWarning *out, size_t cap);
/* A standalone context starts with the checks OFF — a development build
 * turns them on; off costs nothing per frame. */
void kui_set_diagnostics(KuiCtx *ctx, bool on);
/* -- Accessibility (docs/adr/0001-accessibility-as-data.md) ---------------- */
/* The access tree of the last finished frame: fills out with up to cap
 * nodes in tree order (root first) and returns the total count, so a short
 * buffer can be resized and the call repeated. A host wiring its own
 * platform accessibility layer reads it after each frame; a test asserts
 * on it. Never asking costs nothing. */
size_t kui_access_tree(KuiCtx *ctx, KuiAccessNode *out, size_t cap);
/* Says something once, with no node behind it: "Saved", "3 results".
 * `live` is KUI_LIVE_POLITE or KUI_LIVE_ASSERTIVE; KUI_LIVE_OFF and an
 * empty text are both no-ops, the first so a caller can gate politeness
 * without a branch. A region whose message is on screen is KuiSpec.live
 * instead. Call it where the event is handled: called from a frame builder
 * it fires every frame, which the core reports as "announcement-repeated".
 * See docs/adr/0008-live-regions-and-announcements.md. */
void kui_announce(KuiCtx *ctx, KuiStr text, uint32_t live);
/* Drains queued announcements into out (up to cap; the rest are dropped, so
 * size it generously) and returns the count. Drain every frame whether or
 * not assistive technology is attached and discard what you cannot deliver
 * — an announcement kept is an announcement said minutes late. kui_run
 * does this itself. */
size_t kui_take_announcements(KuiCtx *ctx, KuiAnnouncement *out, size_t cap);
/* A request from assistive technology on a node: one KUI_ACCESS_* bit the
 * node advertises, with value the new text for KUI_ACCESS_SET_VALUE (empty
 * otherwise). Resolved like its pointer/keyboard equivalent: a click emits
 * the node's payload, focus lands on an editor, a slider nudge arrives as
 * a {kind="access", action, tag} event. */
void kui_input_access(KuiCtx *ctx, uint64_t key, uint32_t action, KuiStr value);
/* The laid-out text of editor node key as runs (KuiAccessRun): fills out
 * with up to cap of them, returns the total. */
size_t kui_access_runs(KuiCtx *ctx, uint64_t key, KuiAccessRun *out, size_t cap);
/* A text request on an editor: KUI_ACCESS_SET_TEXT_SELECTION with the
 * selection as run positions (anchor the end that stays, focus the caret),
 * or KUI_ACCESS_REPLACE_SELECTED_TEXT / KUI_ACCESS_SET_VALUE with value. A
 * built-in editor applies it (a changed event follows an edit); a custom
 * editor gets it as a {kind="access", action, anchor={line, offset},
 * focus={line, offset}, text, tag} event to apply itself. */
void kui_input_access_text(KuiCtx *ctx, uint64_t key, uint32_t action,
                           uint64_t anchor_run, uint32_t anchor_char,
                           uint64_t focus_run, uint32_t focus_char, KuiStr value);
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
/* Pointers valid until the next kui_frame_begin on this context. False -
 * writing nothing - for a NULL out or one whose `size` says it predates
 * this library; it returned void before that check existed, so a host that
 * ignores the result still compiles. */
bool kui_draw_data(KuiCtx *ctx, KuiDrawData *out);

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

/* -- Extension ABI: C as the guest rather than the host ------------------
 *
 * The other direction from everything above. A host that already owns the
 * window - a Rust app, or anything else driving a Core - loads a shared
 * library and gives it a share of each frame: it draws into the host's tree,
 * keeps its own state, and gets back the events its own nodes emitted and
 * no others. Same deal a Lua extension gets
 * (examples/lua/panel.lua), and the loader on the host's side is
 * kui_ffi::CExtension.
 *
 * YOU define these six; the library only calls them. Two are required -
 * kui_ext_abi and kui_ext_view - and the host refuses to load a plugin
 * without either. The other four are optional, and a missing one is not an
 * error:
 *
 *   kui_ext_abi      REQUIRED. Your KUI_ABI_VERSION. The host refuses a
 *                    mismatch, which is the check a C host makes for itself
 *                    against kui_abi_version(), and it refuses absence the
 *                    same way: a plugin built against a header from before
 *                    this symbol existed is exactly the mismatched plugin
 *                    the check is for, so "absent" cannot mean "unchecked".
 *   kui_ext_name     A name for logs; a NUL-terminated static string. Absent
 *                    = the library's file stem.
 *   kui_ext_init     Your state, handed back to every call below. Absent =
 *                    NULL, which is fine for a stateless panel.
 *   kui_ext_view     REQUIRED. Called once per frame with a context
 *                    borrowing the host's frame. Call the kui_open /
 *                    kui_text / kui_close builders on it; the nodes are
 *                    tagged with the origin the host assigned you. It is
 *                    alive for that one call only - store nothing - and the
 *                    input, frame and draw entry points do not apply to it,
 *                    since the host drives those.
 *   kui_ext_on_event One event of yours, payload borrowed for the call.
 *   kui_ext_free     Your state, at unload.
 *
 * You link against nothing: leave every kui_* symbol undefined and let it
 * resolve from the host executable at load, the way a Lua C module resolves
 * lua_*. That asks one thing of the *host*, which crates/kui-ffi/build.rs
 * does for this crate's examples: link with -rdynamic / --export-dynamic, so
 * that the kui_* symbols in its binary are also in the dynamic symbol table
 * the loader reads. examples/c/build.sh builds your side.
 */
typedef void (*KuiExtViewFn)(void *user, KuiCtx *ctx);
typedef void (*KuiExtEventFn)(void *user, const KuiEvent *ev);
uint32_t kui_ext_abi(void);
const char *kui_ext_name(void);
void *kui_ext_init(void);
void kui_ext_view(void *user, KuiCtx *ctx);
void kui_ext_on_event(void *user, const KuiEvent *ev);
void kui_ext_free(void *user);

#ifdef __cplusplus
}
#endif

#endif /* KUI_H */
